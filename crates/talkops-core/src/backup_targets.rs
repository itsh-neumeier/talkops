//! Backup targets (ADR 0031, ADR 0036): backup jobs that copy the backup
//! archives to a storage target (`storage_targets`). Each job has its own
//! schedule, retention, recordings setting and an optional sub-folder. The
//! server copies the archives with rclone (`talkops_api::backup_targets`).

use std::collections::HashSet;

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::crypto::SecretBox;
use crate::error::{CoreError, CoreResult};
// The connection types moved to `storage_targets` (ADR 0036).
use crate::storage_targets::{self, join_path};
pub use crate::storage_targets::{
    FtpTls, S3_PROVIDERS, TargetConfig, TargetConnection, TargetSecret, WEBDAV_VENDORS,
    normalize_path,
};
use crate::tenant::TenantId;

/// When a target receives a new archive (local time of the container).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Schedule {
    /// Bit 0 = Monday ... bit 6 = Sunday. 0 = only manual runs.
    pub weekdays: i16,
    /// Time of the first run of the day.
    pub hour: i16,
    pub minute: i16,
    /// 0 = once a day; otherwise repeat every N hours until midnight.
    pub every_hours: i16,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            weekdays: 127,
            hour: 3,
            minute: 30,
            every_hours: 0,
        }
    }
}

impl Schedule {
    pub fn validate(&self) -> CoreResult<()> {
        if !(0..=127).contains(&self.weekdays)
            || !(0..=23).contains(&self.hour)
            || !(0..=59).contains(&self.minute)
            || !(0..=23).contains(&self.every_hours)
        {
            return Err(CoreError::Validation("invalid schedule".into()));
        }
        Ok(())
    }

    fn runs_on(&self, day: NaiveDate) -> bool {
        self.weekdays & (1 << day.weekday().num_days_from_monday()) != 0
    }

    /// Run times of one day, in order.
    pub fn times(&self) -> Vec<NaiveTime> {
        let first = u32::from(self.hour.clamp(0, 23) as u16) * 60
            + u32::from(self.minute.clamp(0, 59) as u16);
        let step = u32::from(self.every_hours.clamp(0, 23) as u16) * 60;
        let mut out = Vec::new();
        let mut at = first;
        while at < 24 * 60 {
            out.push(NaiveTime::from_hms_opt(at / 60, at % 60, 0).expect("valid time"));
            if step == 0 {
                break;
            }
            at += step;
        }
        out
    }

    /// The latest planned run at or before `now`, on the same day.
    pub fn latest(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        if !self.runs_on(now.date()) {
            return None;
        }
        self.times()
            .into_iter()
            .rev()
            .find(|t| *t <= now.time())
            .map(|t| now.date().and_time(t))
    }

    /// The next planned run after `now`, within the next eight days.
    pub fn next(&self, now: NaiveDateTime) -> Option<NaiveDateTime> {
        (0..8).find_map(|offset| {
            let day = now.date() + Duration::days(offset);
            if !self.runs_on(day) {
                return None;
            }
            self.times()
                .into_iter()
                .map(|t| day.and_time(t))
                .find(|at| *at > now)
        })
    }

    /// Whether a run is due: a planned time has passed since `since`
    /// (the last run, or the creation of the target).
    pub fn due(&self, since: NaiveDateTime, now: NaiveDateTime) -> bool {
        self.latest(now).is_some_and(|at| at > since)
    }
}

/// Which archives a target keeps (similar to the Synology "smart recycle").
/// An archive stays if one of the rules selects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Retention {
    /// Always keep the latest N archives (at least 1).
    pub keep_last: i16,
    /// Keep the latest archive of each day for N days.
    pub keep_daily: i16,
    /// Keep the latest archive of each week for N weeks.
    pub keep_weekly: i16,
    /// Keep the latest archive of each month for N months.
    pub keep_monthly: i16,
    /// Keep the latest archive of each year for N years.
    pub keep_yearly: i16,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            keep_last: 3,
            keep_daily: 7,
            keep_weekly: 4,
            keep_monthly: 12,
            keep_yearly: 0,
        }
    }
}

impl Retention {
    pub fn validate(&self) -> CoreResult<()> {
        if !(1..=1000).contains(&self.keep_last)
            || !(0..=3660).contains(&self.keep_daily)
            || !(0..=520).contains(&self.keep_weekly)
            || !(0..=1200).contains(&self.keep_monthly)
            || !(0..=100).contains(&self.keep_yearly)
        {
            return Err(CoreError::Validation("invalid retention".into()));
        }
        Ok(())
    }

    /// Indices of the archives to keep. `stamps` are the creation times of
    /// the archives (local time, from their names); `now` is the local time.
    pub fn keep(&self, stamps: &[NaiveDateTime], now: NaiveDateTime) -> HashSet<usize> {
        let mut order: Vec<usize> = (0..stamps.len()).collect();
        order.sort_by(|a, b| stamps[*b].cmp(&stamps[*a]));
        let mut kept: HashSet<usize> = order
            .iter()
            .take(self.keep_last.max(1) as usize)
            .copied()
            .collect();
        // Archives from the future (clock change) are never deleted.
        kept.extend(order.iter().filter(|i| stamps[**i] > now).copied());

        let today = now.date();
        let month_index = |d: NaiveDate| d.year() * 12 + d.month0() as i32;
        let mut bucket = |period: i16, key: &dyn Fn(NaiveDate) -> Option<i64>| {
            if period <= 0 {
                return;
            }
            let mut seen = HashSet::new();
            for i in &order {
                if let Some(k) = key(stamps[*i].date())
                    && seen.insert(k)
                {
                    kept.insert(*i);
                }
            }
        };
        let days = i64::from(self.keep_daily);
        bucket(self.keep_daily, &|d| {
            ((today - d).num_days() < days).then(|| d.num_days_from_ce().into())
        });
        let weeks = i64::from(self.keep_weekly);
        bucket(self.keep_weekly, &|d| {
            let monday =
                |d: NaiveDate| d - Duration::days(d.weekday().num_days_from_monday() as i64);
            let age = (monday(today) - monday(d)).num_days() / 7;
            (age < weeks).then(|| monday(d).num_days_from_ce().into())
        });
        let months = i32::from(self.keep_monthly);
        bucket(self.keep_monthly, &|d| {
            (month_index(today) - month_index(d) < months).then(|| month_index(d).into())
        });
        let years = i32::from(self.keep_yearly);
        bucket(self.keep_yearly, &|d| {
            (today.year() - d.year() < years).then(|| d.year().into())
        });
        kept
    }
}

/// A backup job as shown in the UI. The connection fields come from the
/// storage target and are read-only here.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct BackupTarget {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    /// The storage target that receives the archives.
    pub storage_target_id: Uuid,
    pub storage_name: String,
    /// Sub-folder below the folder of the storage target; `""` = that folder.
    pub folder: String,
    /// Protocol of the storage target.
    pub config: TargetConfig,
    /// Folder of the archives: the storage target folder plus `folder`.
    pub path: String,
    pub allow_insecure: bool,
    pub bwlimit_kbps: Option<i32>,
    /// The storage target has a password or key.
    pub has_secret: bool,
    pub include_recordings: bool,
    pub schedule: Schedule,
    pub retention: Retention,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_file: Option<String>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct BackupTargetInput {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub storage_target_id: Uuid,
    /// Optional sub-folder below the folder of the storage target.
    #[serde(default)]
    pub folder: String,
    #[serde(default = "yes")]
    pub include_recordings: bool,
    #[serde(default)]
    pub schedule: Schedule,
    #[serde(default)]
    pub retention: Retention,
}

fn yes() -> bool {
    true
}

#[derive(FromRow)]
struct Row {
    id: Uuid,
    name: String,
    enabled: bool,
    storage_target_id: Uuid,
    storage_name: String,
    folder: String,
    settings: Json<TargetConfig>,
    storage_path: String,
    allow_insecure: bool,
    bwlimit_kbps: Option<i32>,
    has_secret: bool,
    include_recordings: bool,
    weekdays: i16,
    hour: i16,
    minute: i16,
    every_hours: i16,
    keep_last: i16,
    keep_daily: i16,
    keep_weekly: i16,
    keep_monthly: i16,
    keep_yearly: i16,
    last_run_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
    last_file: Option<String>,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<Row> for BackupTarget {
    fn from(r: Row) -> Self {
        Self {
            id: r.id,
            name: r.name,
            enabled: r.enabled,
            storage_target_id: r.storage_target_id,
            storage_name: r.storage_name,
            path: join_path(&r.storage_path, &r.folder),
            folder: r.folder,
            config: r.settings.0,
            allow_insecure: r.allow_insecure,
            bwlimit_kbps: r.bwlimit_kbps,
            has_secret: r.has_secret,
            include_recordings: r.include_recordings,
            schedule: Schedule {
                weekdays: r.weekdays,
                hour: r.hour,
                minute: r.minute,
                every_hours: r.every_hours,
            },
            retention: Retention {
                keep_last: r.keep_last,
                keep_daily: r.keep_daily,
                keep_weekly: r.keep_weekly,
                keep_monthly: r.keep_monthly,
                keep_yearly: r.keep_yearly,
            },
            last_run_at: r.last_run_at,
            last_success_at: r.last_success_at,
            last_file: r.last_file,
            last_error: r.last_error,
            created_at: r.created_at,
        }
    }
}

const SELECT: &str = "SELECT b.id, b.name, b.enabled, b.storage_target_id,
    s.name AS storage_name, b.folder, s.settings, s.path AS storage_path, s.allow_insecure,
    s.bwlimit_kbps, s.secret_enc IS NOT NULL AS has_secret, b.include_recordings,
    b.weekdays, b.hour, b.minute, b.every_hours, b.keep_last, b.keep_daily, b.keep_weekly,
    b.keep_monthly, b.keep_yearly, b.last_run_at, b.last_success_at, b.last_file,
    b.last_error, b.created_at
    FROM backup_targets b JOIN storage_targets s ON s.id = b.storage_target_id";

pub async fn list(pool: &PgPool, tenant: TenantId) -> CoreResult<Vec<BackupTarget>> {
    let sql = format!("{SELECT} WHERE b.tenant_id = $1 ORDER BY b.name");
    let rows: Vec<Row> = sqlx::query_as(&sql).bind(tenant).fetch_all(pool).await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<BackupTarget> {
    let sql = format!("{SELECT} WHERE b.tenant_id = $1 AND b.id = $2");
    let row: Row = sqlx::query_as(&sql)
        .bind(tenant)
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(row.into())
}

/// The connection of a job: the storage target with decrypted secrets and
/// the folder of the job.
pub async fn connection(
    pool: &PgPool,
    tenant: TenantId,
    secrets: &SecretBox,
    id: Uuid,
) -> CoreResult<(BackupTarget, TargetConnection)> {
    let target = get(pool, tenant, id).await?;
    let (_, mut conn) =
        storage_targets::connection(pool, tenant, secrets, target.storage_target_id).await?;
    conn.path = target.path.clone();
    Ok((target, conn))
}

/// Checks an input; returns the normalized sub-folder.
pub fn check(input: &BackupTargetInput) -> CoreResult<String> {
    storage_targets::check_name(&input.name)?;
    input.schedule.validate()?;
    input.retention.validate()?;
    normalize_path(&input.folder, false)
}

pub async fn create(
    pool: &PgPool,
    tenant: TenantId,
    input: &BackupTargetInput,
) -> CoreResult<BackupTarget> {
    let folder = check(input)?;
    let id = write(pool, tenant, None, input, &folder).await?;
    get(pool, tenant, id).await
}

pub async fn update(
    pool: &PgPool,
    tenant: TenantId,
    id: Uuid,
    input: &BackupTargetInput,
) -> CoreResult<BackupTarget> {
    let folder = check(input)?;
    write(pool, tenant, Some(id), input, &folder).await?;
    get(pool, tenant, id).await
}

async fn write(
    pool: &PgPool,
    tenant: TenantId,
    id: Option<Uuid>,
    input: &BackupTargetInput,
    folder: &str,
) -> CoreResult<Uuid> {
    let sql = match id {
        None => {
            "INSERT INTO backup_targets (tenant_id, id, name, enabled, storage_target_id, folder,
                 include_recordings, weekdays, hour, minute, every_hours,
                 keep_last, keep_daily, keep_weekly, keep_monthly, keep_yearly)
             VALUES ($1, COALESCE($2, gen_random_uuid()), $3, $4, $5, $6, $7, $8, $9, $10, $11,
                 $12, $13, $14, $15, $16)
             RETURNING id"
        }
        Some(_) => {
            "UPDATE backup_targets SET name = $3, enabled = $4, storage_target_id = $5,
                 folder = $6, include_recordings = $7, weekdays = $8, hour = $9, minute = $10,
                 every_hours = $11, keep_last = $12, keep_daily = $13, keep_weekly = $14,
                 keep_monthly = $15, keep_yearly = $16, updated_at = now()
             WHERE tenant_id = $1 AND id = $2 RETURNING id"
        }
    };
    let id: Uuid = sqlx::query_scalar(sql)
        .bind(tenant)
        .bind(id)
        .bind(input.name.trim())
        .bind(input.enabled)
        .bind(input.storage_target_id)
        .bind(folder)
        .bind(input.include_recordings)
        .bind(input.schedule.weekdays)
        .bind(input.schedule.hour)
        .bind(input.schedule.minute)
        .bind(input.schedule.every_hours)
        .bind(input.retention.keep_last)
        .bind(input.retention.keep_daily)
        .bind(input.retention.keep_weekly)
        .bind(input.retention.keep_monthly)
        .bind(input.retention.keep_yearly)
        .fetch_one(pool)
        .await
        .map_err(|err| match &err {
            sqlx::Error::Database(db)
                if db.constraint() == Some("backup_targets_folder_unique") =>
            {
                CoreError::Conflict(
                    "another backup job already uses this folder of the storage target".into(),
                )
            }
            _ => err.into(),
        })?;
    Ok(id)
}

pub async fn delete(pool: &PgPool, tenant: TenantId, id: Uuid) -> CoreResult<()> {
    let done = sqlx::query("DELETE FROM backup_targets WHERE tenant_id = $1 AND id = $2")
        .bind(tenant)
        .bind(id)
        .execute(pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

/// Records the outcome of a run (`Ok(file)` or `Err(message)`).
pub async fn record_run(pool: &PgPool, id: Uuid, result: Result<&str, &str>) -> CoreResult<()> {
    let (file, error) = match result {
        Ok(f) => (Some(f), None),
        Err(e) => (None, Some(e.chars().take(1000).collect::<String>())),
    };
    sqlx::query(
        "UPDATE backup_targets SET last_run_at = now(),
             last_success_at = CASE WHEN $2::text IS NULL THEN last_success_at ELSE now() END,
             last_file = COALESCE($2, last_file), last_error = $3
         WHERE id = $1",
    )
    .bind(id)
    .bind(file)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn schedule_times_and_due() {
        let daily = Schedule::default();
        assert_eq!(daily.times().len(), 1);
        // 2026-10-15 is a Thursday.
        assert_eq!(daily.latest(at("2026-10-15 03:29")), None);
        assert_eq!(
            daily.latest(at("2026-10-15 09:00")),
            Some(at("2026-10-15 03:30"))
        );
        assert!(daily.due(at("2026-10-14 03:30"), at("2026-10-15 03:30")));
        assert!(!daily.due(at("2026-10-15 03:31"), at("2026-10-15 23:00")));
        assert_eq!(
            daily.next(at("2026-10-15 03:30")),
            Some(at("2026-10-16 03:30"))
        );

        let every6 = Schedule {
            weekdays: 0b001_1111, // Monday to Friday
            hour: 1,
            minute: 0,
            every_hours: 6,
        };
        let times: Vec<String> = every6
            .times()
            .iter()
            .map(|t| t.format("%H:%M").to_string())
            .collect();
        assert_eq!(times, ["01:00", "07:00", "13:00", "19:00"]);
        assert_eq!(
            every6.latest(at("2026-10-15 12:59")),
            Some(at("2026-10-15 07:00"))
        );
        assert!(every6.due(at("2026-10-15 07:00"), at("2026-10-15 13:00")));
        // Saturday: no run; the next run is on Monday.
        assert_eq!(every6.latest(at("2026-10-17 12:00")), None);
        assert_eq!(
            every6.next(at("2026-10-16 19:00")),
            Some(at("2026-10-19 01:00"))
        );

        let manual = Schedule {
            weekdays: 0,
            ..Schedule::default()
        };
        assert_eq!(manual.next(at("2026-10-15 00:00")), None);
        assert!(!manual.due(at("2020-01-01 00:00"), at("2026-10-15 12:00")));
    }

    #[test]
    fn smart_retention() {
        let now = at("2026-10-15 04:00");
        // One archive per day for 400 days.
        let stamps: Vec<NaiveDateTime> = (0..400)
            .map(|d| at("2026-10-15 03:30") - Duration::days(d))
            .collect();
        let rule = Retention {
            keep_last: 1,
            keep_daily: 7,
            keep_weekly: 4,
            keep_monthly: 12,
            keep_yearly: 0,
        };
        let kept = rule.keep(&stamps, now);
        let mut days: Vec<String> = kept
            .iter()
            .map(|i| stamps[*i].format("%Y-%m-%d").to_string())
            .collect();
        days.sort();
        days.reverse();
        // Seven days, then Sundays of the last four weeks, then month ends.
        assert_eq!(
            days,
            [
                "2026-10-15",
                "2026-10-14",
                "2026-10-13",
                "2026-10-12",
                "2026-10-11",
                "2026-10-10",
                "2026-10-09",
                "2026-10-04",
                "2026-09-30",
                "2026-09-27",
                "2026-08-31",
                "2026-07-31",
                "2026-06-30",
                "2026-05-31",
                "2026-04-30",
                "2026-03-31",
                "2026-02-28",
                "2026-01-31",
                "2025-12-31",
                "2025-11-30",
            ]
        );

        let count_only = Retention {
            keep_last: 3,
            keep_daily: 0,
            keep_weekly: 0,
            keep_monthly: 0,
            keep_yearly: 0,
        };
        assert_eq!(count_only.keep(&stamps, now).len(), 3);
        assert!(count_only.keep(&stamps, now).contains(&0));

        // Several archives per day: only the latest of the day stays.
        let hourly = vec![
            at("2026-10-15 01:00"),
            at("2026-10-15 03:00"),
            at("2026-10-14 23:00"),
            at("2026-10-14 01:00"),
            at("2026-10-16 00:00"), // future
        ];
        let rule = Retention {
            keep_last: 1,
            keep_daily: 2,
            ..count_only
        };
        let kept = rule.keep(&hourly, now);
        assert_eq!(kept, HashSet::from([1, 2, 4]));
    }

    async fn storage(pool: &PgPool, secrets: &SecretBox, name: &str) -> Uuid {
        let input: storage_targets::StorageTargetInput =
            serde_json::from_value(serde_json::json!({
                "name": name,
                "config": {"kind": "smb", "host": "nas.local", "user": "backup"},
                "path": "talkops",
                "bwlimit_kbps": 500,
                "secret": {"password": "smb-password"}
            }))
            .unwrap();
        storage_targets::create(pool, TenantId::DEFAULT, secrets, &input)
            .await
            .unwrap()
            .id
    }

    fn job(name: &str, storage: Uuid, folder: &str) -> BackupTargetInput {
        BackupTargetInput {
            name: name.into(),
            enabled: true,
            storage_target_id: storage,
            folder: folder.into(),
            include_recordings: false,
            schedule: Schedule::default(),
            retention: Retention::default(),
        }
    }

    #[sqlx::test(migrator = "crate::db::MIGRATOR")]
    async fn jobs_use_a_storage_target(pool: PgPool) {
        let secrets = SecretBox::from_hex(&"ab".repeat(32)).unwrap();
        let tenant = TenantId::DEFAULT;
        let nas = storage(&pool, &secrets, "NAS").await;
        let created = create(&pool, tenant, &job("Nightly", nas, "/daily/"))
            .await
            .unwrap();
        assert_eq!(created.storage_name, "NAS");
        assert_eq!(created.folder, "daily");
        assert_eq!(created.path, "talkops/daily");
        assert_eq!(created.bwlimit_kbps, Some(500));
        assert!(created.has_secret && !created.include_recordings);
        assert!(
            !serde_json::to_string(&created)
                .unwrap()
                .contains("smb-password")
        );
        let (_, conn) = connection(&pool, tenant, &secrets, created.id)
            .await
            .unwrap();
        assert_eq!(conn.path, "talkops/daily");
        assert_eq!(conn.secret.password.as_deref(), Some("smb-password"));

        // Two jobs in one folder would delete the archives of each other.
        let err = create(&pool, tenant, &job("Hourly", nas, "daily"))
            .await
            .unwrap_err();
        assert!(
            matches!(&err, CoreError::Conflict(m) if m.contains("folder")),
            "{err}"
        );
        let hourly = create(&pool, tenant, &job("Hourly", nas, ""))
            .await
            .unwrap();
        assert_eq!(hourly.path, "talkops");
        assert!(matches!(
            create(&pool, tenant, &job("Bad", nas, "../x")).await,
            Err(CoreError::Validation(_))
        ));
        assert!(matches!(
            create(&pool, tenant, &job("Unknown", Uuid::new_v4(), "")).await,
            Err(CoreError::Validation(_))
        ));

        // Move a job to another storage target.
        let usb = storage(&pool, &secrets, "USB").await;
        let mut change = job("Hourly", usb, "");
        change.include_recordings = true;
        let moved = update(&pool, tenant, hourly.id, &change).await.unwrap();
        assert_eq!(moved.storage_name, "USB");
        assert!(moved.include_recordings);
        let users = storage_targets::usages(&pool, tenant).await.unwrap();
        assert_eq!(users[&nas].len(), 1);
        assert_eq!(users[&usb][0].name, "Hourly");

        record_run(&pool, created.id, Err("connection refused"))
            .await
            .unwrap();
        let failed = get(&pool, tenant, created.id).await.unwrap();
        assert_eq!(failed.last_error.as_deref(), Some("connection refused"));
        assert!(failed.last_success_at.is_none());
        record_run(&pool, created.id, Ok("talkops-20261015-033000.tar.gz.age"))
            .await
            .unwrap();
        let ok = get(&pool, tenant, created.id).await.unwrap();
        assert!(ok.last_error.is_none() && ok.last_success_at.is_some());
        assert_eq!(list(&pool, tenant).await.unwrap().len(), 2);
        delete(&pool, tenant, created.id).await.unwrap();
        assert!(matches!(
            delete(&pool, tenant, created.id).await,
            Err(CoreError::NotFound)
        ));
        storage_targets::delete(&pool, tenant, nas).await.unwrap();
    }

    /// Migration 20261104000001 with rows from before the migration: each
    /// backup target becomes a storage target with the same ID and name.
    #[sqlx::test(migrations = false)]
    async fn migration_moves_connections_to_storage_targets(pool: PgPool) {
        const STORAGE: i64 = 20261104000001;
        for m in crate::db::MIGRATOR.iter().filter(|m| m.version < STORAGE) {
            sqlx::raw_sql(&m.sql).execute(&pool).await.unwrap();
        }
        let tenant = TenantId::DEFAULT;
        let nas = Uuid::new_v4();
        let usb = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO backup_targets (id, tenant_id, name, kind, settings, secret_enc, path,
                 allow_insecure, include_recordings, weekdays, every_hours, keep_last,
                 bwlimit_kbps, last_file)
             VALUES ($1, $3, 'NAS', 'smb', '{"kind":"smb","host":"nas"}', 'enc-data',
                     'share/talkops', false, false, 31, 6, 5, 2000, 'talkops-1.tar.gz.age'),
                    ($2, $3, 'USB', 'local', '{"kind":"local"}', NULL,
                     '/var/lib/talkops/targets/usb', true, true, 127, 0, 3, NULL, NULL)"#,
        )
        .bind(nas)
        .bind(usb)
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE roles SET permissions = ARRAY['backups.manage'] WHERE builtin = 'operator'",
        )
        .execute(&pool)
        .await
        .unwrap();
        for m in crate::db::MIGRATOR.iter().filter(|m| m.version >= STORAGE) {
            sqlx::raw_sql(&m.sql).execute(&pool).await.unwrap();
        }

        type StorageRow = (
            Uuid,
            String,
            String,
            Option<String>,
            String,
            bool,
            Option<i32>,
        );
        let storage: Vec<StorageRow> = sqlx::query_as(
            "SELECT id, name, kind, secret_enc, path, allow_insecure, bwlimit_kbps
             FROM storage_targets ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            storage,
            [
                (
                    nas,
                    "NAS".into(),
                    "smb".into(),
                    Some("enc-data".into()),
                    "share/talkops".into(),
                    false,
                    Some(2000)
                ),
                (
                    usb,
                    "USB".into(),
                    "local".into(),
                    None,
                    "/var/lib/talkops/targets/usb".into(),
                    true,
                    None
                ),
            ]
        );
        let jobs = list(&pool, tenant).await.unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id, nas);
        assert_eq!(jobs[0].storage_target_id, nas);
        assert_eq!(jobs[0].path, "share/talkops");
        assert_eq!(jobs[0].folder, "");
        assert_eq!(jobs[0].bwlimit_kbps, Some(2000));
        assert!(jobs[0].has_secret);
        assert_eq!(jobs[0].schedule.weekdays, 31);
        assert_eq!(jobs[0].schedule.every_hours, 6);
        assert_eq!(jobs[0].retention.keep_last, 5);
        assert_eq!(jobs[0].last_file.as_deref(), Some("talkops-1.tar.gz.age"));
        assert!(!jobs[0].include_recordings && jobs[1].include_recordings);
        assert!(jobs[1].allow_insecure && jobs[1].config == TargetConfig::Local);
        let perms: Vec<String> =
            sqlx::query_scalar("SELECT permissions FROM roles WHERE builtin = 'operator'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(perms, ["backups.manage", "storage.manage"]);
        // The used storage target cannot be deleted.
        assert!(
            sqlx::query("DELETE FROM storage_targets WHERE id = $1")
                .bind(nas)
                .execute(&pool)
                .await
                .is_err()
        );
        // A tenant deletion removes both tables.
        sqlx::query("DELETE FROM tenants")
            .execute(&pool)
            .await
            .unwrap();
    }
}
