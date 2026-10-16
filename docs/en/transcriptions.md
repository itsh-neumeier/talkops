# Transcription jobs

## Transcribe a recording again

1. Open the **Call log** and click the **Recording** button of a call.
2. Click **Transcribe again …**.
3. Select a model. The dialog shows for each model:
   - the estimated processing time (**measured** on this server or
     **estimated** from defaults),
   - the estimated cost of a cloud model,
   - the wait for the jobs before it in the queue.
4. Click **Start**.

The current transcript stays visible until the new transcript is complete.
Only one manual transcription of a recording can wait or run at a time.
You need the permission **Transcribe again** and access to the recording.

## Transcriptions page

The menu entry **Transcriptions** shows the transcription jobs. You need the
permission **Own transcription jobs** (jobs that you started or of your own
extensions) or **All transcription jobs**.

The tab **Running and waiting** shows for each job:

- the call, the length of the audio and the model,
- the state: running, waiting, paused, pausing or stopping,
- a progress bar with the percent value,
- the remaining time (running) or the expected end (waiting).

Buttons:

| Button | Effect |
| --- | --- |
| Pause | A waiting job stays in the queue. A running local job stops within about one second and keeps its position. |
| Resume | The job goes back into the queue. A local job continues at the paused position. A cloud job starts again. |
| Stop | The job ends. A manual or second pass keeps the current transcript. A stopped first pass leaves no transcript; a voicemail e-mail is sent without it. |
| Move up | The waiting job goes to the front of the queue. |

The tab **History** shows the jobs of the last 7 days that finished, failed
or stopped (at most the newest 1000 jobs).

### History: parties, sort and filter

The column **Parties** shows the caller and the called party of the call or
the voicemail. The phone book supplies the names, the same as in the
[call log](call-log.md): click a name to open the phone book entry. For an
inbound call, the line below shows the own number that the caller dialed.

- **Sort:** click the column title **Time**, **Duration** or **Result**.
  A second click changes the order (descending, ascending). **Duration** is
  the processing time. Jobs that did not run are at the end.
- **Filter:** a column with a filter has a funnel button. A colored dot
  shows an active filter.

  | Column | Filter |
  | --- | --- |
  | Time | Today, yesterday, or a range of dates in the last 7 days (both days included). |
  | Direction | One or more directions. A voicemail from an extension is internal; other voicemails are inbound. |
  | Model | One or more models of the jobs in the history. |
  | Result | Done, failed, stopped. |

- **Search:** the search field finds text in the names and numbers of the
  parties and in the name of the user who started the job. A search with
  digits only, for example `0911 2193`, also finds `+49 911 2193…`.
- **Reset all filters** clears the filters and the search. The sort order
  does not change.

The browser filters and sorts the loaded history, so each change shows at
once. The page address contains the tab, the filters and the sort order.
A reload or a copied link thus shows the same list. The history reloads
every 30 seconds while the tab is open; the active jobs reload every
2 seconds.

On a narrow screen, the columns *Model* and *Duration* are not visible.

## Resources

With the permission **All transcription jobs**, the page also shows each
media worker:

- **CPU**: the CPU time of the worker and its Whisper processes. 100 % is one
  core.
- **Memory**: the memory use and limit of the container (or of the host).
- **Worker and Whisper**: the memory of the worker processes.
- **Load**: the load average of the last minute.

The page updates every 2 seconds.

## Estimates

The worker measures each complete run: processing time for each second of
recording, for each model. The estimate uses the mean of the first runs and
then follows changes slowly. Resumed runs are not measured.

## API

- `GET /api/v1/recordings/{id}/transcribe`: models with estimates.
- `POST /api/v1/recordings/{id}/transcribe` with
  `{"engine": {"kind": "local", "model": "accurate"}}` or
  `{"engine": {"kind": "cloud", "provider_id": "…", "model": "gpt-transcribe"}}`.
- `GET /api/v1/transcription-jobs` (`?history=true` for the history).
  Each job has `direction`, `caller_number` and `caller_name`. History
  entries also have the resolved parties `caller` and `called` and, for
  inbound calls, `via_number` (the same as `GET /api/v1/calls`).
- `POST /api/v1/transcription-jobs/{id}` with
  `{"action": "pause" | "resume" | "stop" | "prioritize"}`.
- `GET /api/v1/transcription-workers`: resource usage.
