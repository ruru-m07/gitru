export function bridgeVerticalWheel(
  element: HTMLElement,
  scrollElement: HTMLElement,
): () => void {
  const onWheel = (event: WheelEvent) => {
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
    scrollElement.scrollTop += event.deltaY;
    event.preventDefault();
    event.stopPropagation();
  };

  element.addEventListener("wheel", onWheel, { passive: false });
  return () => element.removeEventListener("wheel", onWheel);
}
