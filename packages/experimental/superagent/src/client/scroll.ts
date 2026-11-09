/** Scroll an element into view, tolerating DOM implementations without `scrollIntoView`. */
export function scrollIntoView(element: HTMLElement | null | undefined, options: ScrollIntoViewOptions): void {
  const prototype: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } = Element.prototype
  const scroll = prototype.scrollIntoView
  if (element !== null && element !== undefined && scroll !== undefined) scroll.call(element, options)
}
