/** Read live abort state across an await without treating it as synchronously immutable. */
export const isAborted = (signal: AbortSignal): boolean => signal.aborted
