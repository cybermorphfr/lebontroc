import path from "node:path";
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Image Docker minimale : .next/standalone
  output: "standalone",
  // Monorepo : le file tracing doit partir de la racine du repo
  outputFileTracingRoot: path.join(__dirname, "../.."),
  transpilePackages: ["@lebontroc/api-client"],
  // En dev (pas de Traefik), /api/* est réécrit vers l'API locale pour que
  // les cookies gardent les mêmes chemins qu'en prod.
  async rewrites() {
    const target = process.env.API_PROXY_URL;
    if (!target) return [];
    return [{ source: "/api/:path*", destination: `${target}/:path*` }];
  },
};

export default nextConfig;
