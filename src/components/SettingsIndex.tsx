import { useEffect, useState, type RefObject } from "react";

export function SettingsIndex({ sections }: { sections: RefObject<HTMLDivElement | null> }) {
  const [links, setLinks] = useState<{ id: string; label: string }[]>([]);
  const [current, setCurrent] = useState("");
  useEffect(() => {
    const list = sections.current;
    const main = list?.closest("main");
    if (!list || !main) return;
    let headings: HTMLElement[] = [];
    const position = () => {
      const top = main.getBoundingClientRect().top + 48;
      let id = headings.at(0)?.id;
      for (const el of headings) if (el.getBoundingClientRect().top <= top) id = el.id;
      if (main.scrollTop > 0 && main.scrollHeight - main.scrollTop <= main.clientHeight + 2)
        id = headings.at(-1)?.id;
      setCurrent(id ?? "");
    };
    function refresh() {
      headings = Array.from(list!.querySelectorAll<HTMLElement>("section > h2[id]"));
      setLinks(headings.map((el) => ({ id: el.id, label: el.textContent ?? "" })));
      position();
    }
    refresh();
    const observer = new MutationObserver(refresh);
    observer.observe(list, { childList: true });
    main.addEventListener("scroll", position, { passive: true });
    return () => {
      observer.disconnect();
      main.removeEventListener("scroll", position);
    };
  }, [sections]);
  return (
    <nav
      aria-label="Settings sections"
      className="sticky top-4 hidden self-start border-l pl-4 text-xs md:block"
    >
      <p className="mb-2 font-semibold">In Settings</p>
      {links.map(({ id, label }) => (
        <a
          key={id}
          href={`#${id}`}
          aria-current={current === id ? "location" : undefined}
          className="block py-1.5 text-muted-foreground hover:underline aria-[current=location]:text-foreground"
          onClick={(event) => {
            event.preventDefault();
            const el = document.getElementById(id);
            if (el) {
              el.tabIndex = -1;
              el.focus({ preventScroll: true });
              el.scrollIntoView({ block: "start" });
            }
          }}
        >
          {label}
        </a>
      ))}
    </nav>
  );
}
