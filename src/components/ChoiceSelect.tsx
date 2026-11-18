import type { ComponentProps } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "#components/core/select";

type Choice = { value: string; label: string; disabled?: boolean };

export function ChoiceSelect({
  value,
  options,
  disabled,
  onChange,
  onOpen,
  ...trigger
}: {
  value: string;
  options: Choice[];
  disabled?: boolean;
  onChange(value: string): void;
  onOpen?(): void;
} & Pick<
  ComponentProps<typeof SelectTrigger>,
  "id" | "className" | "aria-describedby" | "aria-label"
>) {
  return (
    <Select
      value={value}
      items={options}
      disabled={disabled}
      onOpenChange={(open) => {
        if (open) onOpen?.();
      }}
      onValueChange={(next) => {
        if (next !== null) onChange(next);
      }}
    >
      <SelectTrigger {...trigger}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map((option) => (
          <SelectItem key={option.value} value={option.value} disabled={option.disabled}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
