import { useEffect, useRef, useState } from "react";
import { Copy, Trash, X } from "@phosphor-icons/react";
import { Button } from "#components/core/button";
import { NativeSelect } from "#components/core/native-select";
import type { HistoryListing, HistorySource, Retention } from "../engine/history";
import type { Transcript } from "../engine/types";

const labels: [Retention, string][] = [
  ["off", "Off"],
  ["day", "24 hours"],
  ["week", "7 days"],
  ["month", "30 days"],
  ["forever", "Forever"],
];

export function HistoryPage({
  source,
  result,
  demo,
}: {
  source: HistorySource;
  result: Transcript | null;
  demo: boolean;
}) {
  const [listing, setListing] = useState<HistoryListing | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<number | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [pending, setPending] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [deleting, setDeleting] = useState<{ id: number; text: string } | null>(null);
  const deleteDialog = useRef<HTMLDialogElement>(null);
  const [pruning, setPruning] = useState<{ retention: Retention; count: number } | null>(null);
  const [notice, setNotice] = useState("");
  const search = useRef<HTMLInputElement>(null);
  const detail = listing?.entries.find((entry) => entry.id === selected) ?? listing?.entries[0];

  useEffect(() => {
    if (deleting || pruning) deleteDialog.current?.showModal();
  }, [deleting, pruning]);

  async function applyRetention(retention: Retention, confirmed = 0) {
    setPending(true);
    try {
      const count = await source.setRetention(retention, confirmed);
      if (count) {
        setPruning({ retention, count });
        return;
      }
      deleteDialog.current?.close();
      setNotice("Retention updated.");
      setRefresh((n) => n + 1);
    } catch (error) {
      deleteDialog.current?.close();
      setNotice(String(error));
    } finally {
      setPending(false);
    }
  }

  useEffect(() => {
    let disposed = false;
    async function load() {
      try {
        const next = await source.list(query);
        if (!disposed) setListing(next);
      } catch (error) {
        if (!disposed) setNotice(String(error));
      }
    }
    const timer = setTimeout(() => void load(), query ? 150 : 0);
    const interval = setInterval(() => void load(), 15000);
    return () => {
      disposed = true;
      clearTimeout(timer);
      clearInterval(interval);
    };
  }, [source, query, result, refresh]);

  async function change(action: () => Promise<void>, message: string) {
    setPending(true);
    setNotice("");
    try {
      await action();
      setNotice(message);
      setRefresh((value) => value + 1);
    } catch (error) {
      setNotice(String(error));
    } finally {
      setPending(false);
      setConfirmClear(false);
    }
  }

  return (
    <main
      className="mx-auto flex w-full max-w-5xl flex-1 flex-col gap-6 px-4 py-8 sm:px-8"
      aria-labelledby="history-title"
    >
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h1
          id="history-title"
          tabIndex={-1}
          className="text-2xl font-semibold tracking-tight outline-none"
        >
          History
        </h1>
        <div className="flex flex-wrap items-center gap-2">
          <label htmlFor="retention" className="text-xs text-muted-foreground">
            Keep
          </label>
          <NativeSelect
            id="retention"
            className="w-auto"
            value={listing?.retention ?? "week"}
            disabled={!listing || pending}
            onChange={(event) => void applyRetention(event.target.value as Retention)}
          >
            {labels.map(([value, label]) => (
              <option value={value} key={value}>
                {label}
              </option>
            ))}
          </NativeSelect>
          <Button
            variant="ghost"
            size="sm"
            disabled={!listing?.total || pending}
            onClick={() =>
              confirmClear
                ? void change(() => source.clear(), "History cleared.")
                : setConfirmClear(true)
            }
          >
            <Trash aria-hidden="true" />
            {confirmClear ? "Confirm clear" : "Clear all"}
          </Button>
          {confirmClear && (
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Cancel clear history"
              onClick={() => setConfirmClear(false)}
            >
              <X aria-hidden="true" />
            </Button>
          )}
        </div>
      </div>
      <p className="supporting">
        {demo
          ? "Demo history · kept only for this browser session."
          : listing?.retention === "off"
            ? "New transcripts aren’t saved. Existing entries stay until you delete them."
            : "Saved on this device. Audio isn’t kept."}
      </p>
      {listing?.error && (
        <p role="alert" className="supporting text-destructive">
          {listing.error}
        </p>
      )}
      <div className="grid min-h-80 overflow-hidden rounded-sm border md:grid-cols-[minmax(220px,2fr)_3fr]">
        <section
          aria-label="Transcripts"
          className="flex min-w-0 flex-col border-b md:border-r md:border-b-0"
        >
          <div className="border-b p-3">
            <input
              ref={search}
              type="search"
              aria-label="Search history"
              placeholder="Search history"
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
                setSelected(null);
              }}
              className="h-9 w-full rounded-sm border border-input bg-background px-3 text-sm outline-none focus-visible:border-ring"
            />
          </div>
          <ul className="max-h-64 overflow-y-auto md:max-h-[32rem]">
            {listing?.entries.map((entry) => (
              <li key={entry.id}>
                <button
                  className={`flex w-full flex-col gap-2 border-b p-4 text-left text-sm outline-none transition-colors hover:bg-muted focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring ${detail?.id === entry.id ? "bg-accent" : ""}`}
                  aria-current={detail?.id === entry.id ? "true" : undefined}
                  onClick={() => {
                    setSelected(entry.id);
                    setNotice("");
                  }}
                >
                  <span className="line-clamp-2 break-words">{entry.text}</span>
                  <time
                    dateTime={new Date(entry.timestampMs).toISOString()}
                    className="text-xs text-muted-foreground"
                  >
                    {new Date(entry.timestampMs).toLocaleString([], {
                      month: "short",
                      day: "numeric",
                      hour: "numeric",
                      minute: "2-digit",
                    })}
                  </time>
                </button>
              </li>
            ))}
          </ul>
          {!listing?.entries.length && (
            <p className="p-6 text-sm text-muted-foreground">
              {!listing ? "Loading…" : query ? "No matches." : "No transcripts yet."}
            </p>
          )}
          {!!listing && listing.matches > listing.entries.length && (
            <p className="supporting p-3">
              Showing the newest 200 matches. Search to narrow the list.
            </p>
          )}
        </section>
        <section
          className="flex min-w-0 flex-col gap-5 p-5 sm:p-6"
          aria-label="Selected transcript"
        >
          {detail ? (
            <>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <p className="supporting">
                  {new Date(detail.timestampMs).toLocaleString()} ·{" "}
                  {(detail.durationMs / 1000).toFixed(1)}s
                </p>
                <div className="flex items-center gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={pending}
                    onClick={() =>
                      void change(() => source.copy(detail.id), "Copied to clipboard.")
                    }
                  >
                    <Copy aria-hidden="true" />
                    Copy
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label="Delete transcript"
                    disabled={pending}
                    onClick={() => setDeleting({ id: detail.id, text: detail.text })}
                  >
                    <Trash aria-hidden="true" />
                  </Button>
                </div>
              </div>
              <p className="select-text whitespace-pre-wrap break-words text-base leading-7">
                {detail.text}
              </p>
            </>
          ) : (
            <p className="supporting">
              {query ? "Try another search." : "Your completed dictations will appear here."}
            </p>
          )}
        </section>
      </div>
      <p role="status" className="supporting min-h-5">
        {notice}
      </p>
      <dialog
        ref={deleteDialog}
        aria-labelledby="delete-title"
        aria-describedby="delete-warning"
        className="m-auto w-[min(24rem,90vw)] rounded-sm border bg-background p-5 text-foreground backdrop:bg-black/40"
        onKeyDown={(event) => {
          if (event.key === "Escape") event.stopPropagation();
        }}
        onClose={() => {
          setDeleting(null);
          setPruning(null);
        }}
      >
        <h2 id="delete-title" className="font-semibold">
          {pruning
            ? `Delete ${pruning.count} older transcript${pruning.count === 1 ? "" : "s"}?`
            : "Delete this transcript?"}
        </h2>
        <p id="delete-warning" className="supporting my-3">
          {pruning
            ? `Keeping ${labels.find(([value]) => value === pruning.retention)?.[1]} of history removes these entries. This can’t be undone.`
            : "This removes it from History. This can’t be undone."}
        </p>
        <blockquote className="line-clamp-2 break-words text-sm">{deleting?.text}</blockquote>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="outline" autoFocus onClick={() => deleteDialog.current?.close()}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            disabled={(!deleting && !pruning) || pending}
            onClick={() => {
              if (pruning) {
                void applyRetention(pruning.retention, pruning.count);
                return;
              }
              if (!deleting) return;
              const { id } = deleting;
              deleteDialog.current?.close();
              void change(() => source.remove(id), "Transcript deleted.");
              search.current?.focus();
            }}
          >
            Delete
          </Button>
        </div>
      </dialog>
    </main>
  );
}
