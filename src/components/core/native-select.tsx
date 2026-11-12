import * as React from "react";
import { cn } from "#lib/utils";

function NativeSelect({ className, ...props }: React.ComponentProps<"select">) {
  return (
    <select
      data-slot="native-select"
      className={cn(
        "flex h-9 w-full appearance-auto items-center rounded-[var(--radius)] border border-input bg-background px-2.5 py-1 text-base transition-colors duration-150 outline-none focus-visible:border-ring disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive md:text-sm",
        className,
      )}
      {...props}
    />
  );
}

export { NativeSelect };
