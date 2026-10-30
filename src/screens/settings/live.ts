/** Appel asynchrone « en direct » : temporisé, et seule la dernière réponse compte. */
export type Live<A> = { run(arg: A): void; cancel(): void };

export function liveCheck<A, R>(
  fn: (arg: A) => Promise<R>,
  onResult: (r: R) => void,
  onError: (message: string) => void,
  delay = 250,
): Live<A> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seq = 0;
  const cancel = () => {
    seq++;
    clearTimeout(timer);
  };
  const run = (arg: A) => {
    cancel();
    const mine = seq;
    timer = setTimeout(() => {
      fn(arg).then(
        (r) => mine === seq && onResult(r),
        (e) => mine === seq && onError(String(e)),
      );
    }, delay);
  };
  return { run, cancel };
}
