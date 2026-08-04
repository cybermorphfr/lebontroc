"use client";

import { useId } from "react";

type SegmentedOption = { value: string; label: string };

/** Sélecteur pilule segmenté du DS — radiogroup accessible. */
export function Segmented({
  options,
  value,
  onChange,
  label,
}: {
  options: SegmentedOption[];
  value: string;
  onChange: (value: string) => void;
  label?: string;
}) {
  const group = useId();
  return (
    <div className="flex flex-col">
      {label ? <span className="mb-1.5 text-xs text-encre/70">{label}</span> : null}
      <div
        role="radiogroup"
        aria-label={label}
        className="inline-flex w-fit max-w-full overflow-x-auto rounded-full border border-neutre-300"
      >
        {options.map((option, index) => {
          const checked = value === option.value;
          return (
            <label
              key={option.value}
              className={`inline-flex cursor-pointer items-center gap-1.5 whitespace-nowrap px-3 py-1.5 text-[13px] transition-colors ${
                index > 0 ? "border-l border-neutre-300" : ""
              } ${checked ? "bg-[#c67139] text-creme" : "hover:bg-encre/7"} has-[input:focus-visible]:outline-2 has-[input:focus-visible]:-outline-offset-2 has-[input:focus-visible]:outline-terracotta-500`}
            >
              <input
                type="radio"
                name={group}
                checked={checked}
                onChange={() => onChange(option.value)}
                className="pointer-events-none absolute h-0 w-0 opacity-0"
              />
              {option.label}
            </label>
          );
        })}
      </div>
    </div>
  );
}
