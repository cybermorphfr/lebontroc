import path from "node:path";
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Image Docker minimale : .next/standalone
  output: "standalone",
  // Monorepo : le file tracing doit partir de la racine du repo
  outputFileTracingRoot: path.join(__dirname, "../.."),
  transpilePackages: ["@lebontroc/api-client"],
};

export default nextConfig;
