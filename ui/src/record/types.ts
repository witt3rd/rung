/** One line of a host's record: numbered without gaps, `kind` names the event. The kinds and fields are
 *  listed in docs/rung-host.md (the record table); fields not read here are kept as they were. */
export interface Line {
  seq: number;
  at: number;
  kind: string;
  // The record is JSON; a field is read where it is used and checked there.
  // biome-ignore lint: open record shape
  [field: string]: any;
}
