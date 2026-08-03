import { expect, test } from "@playwright/test";

// F0.1 — Scénario : le squelette est vivant de bout en bout
// Étant donné l'environnement déployé
// Quand je visite la page d'accueil
// Alors je vois le statut "API opérationnelle" et la version du build

test("le squelette est vivant de bout en bout", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("API opérationnelle")).toBeVisible();
  await expect(page.getByText("Version du build")).toBeVisible();
  // La version affichée n'est pas le tiret de repli.
  const version = page
    .locator("dd")
    .filter({ hasText: /^0\.\d+\.\d+/ })
    .first();
  await expect(version).toBeVisible();
});

test("la marque et le titre sont en place", async ({ page }) => {
  await page.goto("/");

  await expect(page).toHaveTitle(/Lebontroc/);
  await expect(
    page.getByRole("heading", { level: 1, name: "Lebontroc" }),
  ).toBeVisible();
});
