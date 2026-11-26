/** Closed unions must handle every variant before reaching this branch. */
export function assertNever(value:never):never {
 throw new Error(`未対応の値です: ${String(value)}`);
}
