/** What a paged list offers. `shown` is how far the owner has opened it, `need` how far a link into it requires it to be open (the list is held
 *  open at least that far), `page` how many a step adds. Show less is offered only when it would change what is shown: the owner has opened more
 *  than one page. One rule for every list. */
export function pageControls(a: { shown: number; need: number; total: number; page: number }): { count: number; more: number; all: boolean; less: boolean } {
  const count = Math.max(a.shown, a.need);
  const left = Math.max(0, a.total - count);
  return { count, more: Math.min(a.page, left), all: left > 0, less: a.shown > a.page };
}
