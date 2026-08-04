import type { ButtonHTMLAttributes, ReactNode } from "react";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "sage";
  size?: "md" | "lg";
  block?: boolean;
  icon?: ReactNode;
};

const base =
  "inline-flex cursor-pointer items-center justify-center gap-1.5 whitespace-nowrap rounded-full border border-transparent font-display leading-tight transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-45 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-terracotta-500";

const variants = {
  primary: "bg-terracotta-500 text-creme hover:bg-terracotta-600 active:bg-terracotta-700 bg-[#c67139]",
  secondary: "border-neutre-300 text-encre hover:bg-encre/7 active:bg-encre/14",
  ghost: "px-1.5 text-terracotta-700 hover:bg-terracotta-500/10 active:bg-terracotta-500/18",
  sage: "bg-sauge-700 text-creme hover:bg-sauge-700 active:bg-sauge-800 bg-[#7a8a5e]",
} as const;

const sizes = {
  md: "px-4 py-2 text-sm",
  lg: "px-7 py-3 text-base",
} as const;

/** Bouton pilule Lebontroc — un seul `primary` par écran. */
export function Button({
  variant = "primary",
  size = "md",
  block = false,
  icon,
  className = "",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      className={`${base} ${variants[variant]} ${sizes[size]} ${block ? "w-full" : ""} ${className}`}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
}
