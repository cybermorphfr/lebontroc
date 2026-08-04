import type { HTMLAttributes } from "react";

type TagProps = HTMLAttributes<HTMLSpanElement> & {
  variant?: "accent" | "accent-2" | "neutral" | "outline";
};

const variants = {
  accent: "bg-terracotta-100 text-terracotta-800",
  "accent-2": "bg-sauge-100 text-sauge-800",
  neutral: "bg-neutre-100 text-neutre-700",
  outline: "border border-terracotta-500 text-terracotta-700",
} as const;

/** Label pilule pour états et catégories. */
export function Tag({ variant = "neutral", className = "", children, ...rest }: TagProps) {
  return (
    <span
      className={`inline-flex flex-none items-center gap-1 whitespace-nowrap rounded-full px-2.5 py-0.5 text-[11px] tracking-wide ${variants[variant]} ${className}`}
      {...rest}
    >
      {children}
    </span>
  );
}
