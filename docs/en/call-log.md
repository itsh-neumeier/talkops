# Call log

The **Call log** shows the finished calls. Admins and operators see all
calls. Users see the calls of their own extensions.

## Columns

| Column | Content |
| --- | --- |
| Time | Start of the call. |
| Direction | Inbound, outbound or internal. |
| From | The caller: an extension number with its name, or an external number. |
| To | The called party. For an answered inbound call: the answering extension, the own number that the caller dialed, and the extension name, for example `Ext. 101 via +4995431241` and `Familie Zweig`. |
| Duration | Talk time of an answered call. |
| Result | `OK`, `busy/declined` or `not answered`. |
| Recording | Play button for a recorded call. |

## Phone book names

The check box **Show numbers through the phone book** is on by default.

- On: an external number with a phone book entry shows the entry name first
  and the number below it. Click the name to open the phone book entry.
  When more than one entry has the number, the phone book of the own
  extension supplies the name (the same rule as for
  [active calls](active-calls.md)).
- Off: the list shows the number first and the caller name from the call
  record below it.

The browser keeps this setting.

## Sort

Click the column title **Time** or **Duration** to sort by this column.
A second click changes the order (descending, ascending). The arrow shows
the current order. The server sorts, so the order applies to all calls,
not only to the loaded rows.

## Filter

A column with a filter has a funnel button. Click it to open the filter
panel of this column. A colored dot shows an active filter.

| Column | Filter |
| --- | --- |
| Time | Today, 7 days, 30 days, or a range of dates (both days included). |
| Direction | One or more directions. |
| From | Text in the number, the name, the extension or the phone book name. |
| To | Text in the number, the answering extension, the dialed own number or the phone book name. |
| Result | Answered, not answered, busy/declined. |
| Recording | All, with recording, with transcript, without recording. |

The text filters start after a short typing pause. **Reset** clears the
filter of one column. **Reset all filters** clears all filters and the
search, but keeps the sort order.

The page address contains the filters and the sort order. A reload or a
copied link thus shows the same list.

## Load more

The list loads 50 calls at a time. The line below the table shows the
number of loaded calls and the number of all matching calls. Click
**Load more** to load the next calls.

## API

`GET /api/v1/calls` accepts these query parameters: `since`, `until`
(times), `direction`, `result` (lists separated by commas), `from`, `to`,
`recording` (`with`, `transcript`, `without`), `search`, `sort` (`time`,
`duration`), `order` (`asc`, `desc`), `offset` and `limit` (maximum 500).
The header `X-Total-Count` has the number of all matching calls. Each call
also has the resolved parties `caller` and `called` and, for inbound calls,
`via_number`.
