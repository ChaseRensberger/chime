import { GithubLogo } from "@phosphor-icons/react";
import { buttonVariants } from "#components/core/button-variants";
import { InstallPanel } from "./components/InstallPanel";

const repository = "https://github.com/ChaseRensberger/chime";
const bars = Array.from({ length: 65 }, (_, i) => {
  const envelope = Math.sin((i / 64) * Math.PI) ** 1.5;
  return 4 + envelope * (24 + Math.abs(Math.sin(i * 1.7)) * 100);
});

export function App() {
  return (
    <div className="mx-auto flex min-h-dvh w-full max-w-5xl flex-col px-4 sm:px-8">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:top-4 focus:bg-background focus:p-3"
      >
        Skip to content
      </a>
      <header className="flex items-center justify-between gap-4 border-b py-2 sm:py-3">
        <a
          href="/"
          aria-label="Chime home"
          className="inline-flex min-h-11 items-center gap-2 text-lg font-semibold tracking-tight"
        >
          Chime
        </a>
        <nav aria-label="Main navigation" className="flex items-center gap-2 sm:gap-4">
          <a href={repository} className={`${buttonVariants({ variant: "ghost" })} min-h-11`}>
            <GithubLogo aria-hidden="true" /> GitHub
          </a>
        </nav>
      </header>

      <main id="main" className="flex flex-1 flex-col items-center justify-center py-4">
        <section
          aria-labelledby="hero-title"
          className="flex w-full max-w-xl flex-col items-center gap-4 text-center"
        >
          <div
            aria-hidden="true"
            className="flex h-[clamp(3rem,10dvh,6rem)] w-full max-w-xl items-center justify-center gap-0.5 px-4 text-link sm:h-[clamp(3rem,16dvh,10rem)] sm:gap-1"
          >
            {bars.map((height, i) => (
              <span
                key={i}
                className="max-w-1 min-w-0 flex-1 bg-current"
                style={{ height: `${(height / 128) * 100}%` }}
              />
            ))}
          </div>
          <div className="flex flex-col items-center gap-3">
            <p className="text-xs tracking-widest text-muted-foreground uppercase">
              Local voice dictation
            </p>
            <h1
              id="hero-title"
              className="text-2xl leading-8 font-semibold tracking-tight sm:text-4xl sm:leading-10"
            >
              A little less typing.
            </h1>
            <p className="max-w-md text-base leading-7 text-muted-foreground">
              Press a shortcut. Say what you mean.
              <br className="hidden sm:block" /> Chime turns your voice into text, on your device.
            </p>
          </div>
          <InstallPanel repository={repository} />
        </section>
      </main>

      <footer className="flex items-center justify-center border-t py-2 text-xs text-muted-foreground sm:py-3">
        <a
          href="https://rensberger.xyz"
          className="inline-flex min-h-11 items-center gap-1 underline-offset-4 hover:text-foreground hover:underline"
        >
          © {new Date().getFullYear()} Rensberger
        </a>
      </footer>
    </div>
  );
}
