/**
 * Which page of a long document is the one being read: the last page whose top has reached `line`
 * (a line a little below the top of the visible zone), or the first page while none has. Pages are
 * stacked from top to bottom, so their tops only grow and a binary search finds it among hundreds.
 *
 * @param topOf the top of the page at a 0-based index, in the same unit as `line`
 * @param count how many pages there are
 * @returns a 1-based page number, `1` for an empty list
 */
export function pageAtLine(topOf: (index: number) => number, count: number, line: number): number {
  let low = 0;
  let high = count - 1;
  let found = 0;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (topOf(middle) <= line) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found + 1;
}
