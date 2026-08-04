import { ResendVerification } from "@/components/ResendVerification";

/** Bandeau affiché tant que l'e-mail n'est pas vérifié. */
export function VerifyEmailBanner({ message }: { message?: string }) {
  return (
    <div className="mx-4 mb-2 flex flex-wrap items-center justify-center gap-x-3 gap-y-1 rounded-full bg-terracotta-100 px-5 py-2.5 text-sm text-terracotta-800 sm:mx-12 lg:mx-24">
      <span>
        {message ??
          "Vérifie ton e-mail pour troquer l'esprit tranquille — regarde ta boîte mail."}
      </span>
      <ResendVerification />
    </div>
  );
}
