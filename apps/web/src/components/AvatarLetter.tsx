/** Avatar-lettre du DS : initiale du pseudo dans un cercle terracotta. */
export function AvatarLetter({
  pseudo,
  size = "md",
}: {
  pseudo: string;
  size?: "sm" | "md" | "lg";
}) {
  const sizes = {
    sm: "size-8 text-sm",
    md: "size-12 text-xl",
    lg: "size-16 text-2xl",
  } as const;
  return (
    <span
      aria-hidden
      className={`flex items-center justify-center rounded-full bg-terracotta-100 font-display text-terracotta-800 ${sizes[size]}`}
    >
      {pseudo.charAt(0).toUpperCase()}
    </span>
  );
}
