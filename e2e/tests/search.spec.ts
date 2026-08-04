import { expect, test, type Browser, type Page } from "@playwright/test";

// F2.2 — Scénarios Gherkin « recherche et filtres » :
// tolérance aux fautes, filtres combinés tous respectés.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<void> {
  const email = `cherche-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `cherche${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
  await page.goto("/inscription");
  await page.getByLabel("Pseudo").fill(pseudo);
  await page.getByLabel("E-mail").fill(email);
  await page.getByLabel("Mot de passe", { exact: true }).fill("un-bon-mot-de-passe");
  await page.getByLabel("Code postal").fill(postalCode);
  await page.getByRole("button", { name: "Créer mon compte" }).click();
  await expect(page).toHaveURL(/\/verification$/);

  let messageId = "";
  await expect
    .poll(async () => {
      const response = await fetch(
        `${MAILPIT}/api/v1/search?query=${encodeURIComponent(`to:${email}`)}`,
      );
      const body = (await response.json()) as { messages?: Array<{ ID: string }> };
      messageId = body.messages?.[0]?.ID ?? "";
      return messageId;
    }, { timeout: 15000 })
    .not.toBe("");
  const message = (await (await fetch(`${MAILPIT}/api/v1/message/${messageId}`)).json()) as {
    Text: string;
  };
  const link = message.Text.match(/https?:\/\/[^\s]+verify-email\?token=[A-Za-z0-9_-]+/)?.[0];
  if (!link) throw new Error("lien de vérification introuvable");
  await page.goto(link);
}

async function publishItem(page: Page, title: string) {
  await page.goto("/publier");
  await page.locator('input[type="file"]').setInputFiles([
    { name: "photo.png", mimeType: "image/png", buffer: TINY_PNG },
  ]);
  await page.getByLabel("Titre").fill(title);
  await page.getByRole("button", { name: "Choisis une catégorie" }).click();
  await page.getByRole("button", { name: "Enfants et puériculture" }).click();
  await page.getByRole("button", { name: "Jouets et éveil" }).click();
  await page.getByLabel("Description").fill("Objet de test pour la recherche.");
  await page.getByLabel("Valeur indicative (€)").fill("30");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

async function publishAs(browser: Browser, postalCode: string, title: string) {
  const context = await browser.newContext();
  const page = await context.newPage();
  await verifiedUser(page, postalCode);
  await publishItem(page, title);
  await context.close();
}

test("la recherche tolère les fautes et les filtres combinés sont respectés", async ({
  page,
  browser,
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const proche = `Poussette Bijou${suffixe}`;
  const lointaine = `Poussette Freya${suffixe}`;
  await publishAs(browser, "44300", proche);
  await publishAs(browser, "75001", lointaine);

  await verifiedUser(page, "44000");

  // Tolérance aux fautes : « pousette » trouve les poussettes.
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill("pousette");
  await expect(page.getByText(proche)).toBeVisible();
  await expect(page.getByText(lointaine)).toBeVisible();

  // Filtres combinés : enfants + moins de 10 km + main propre.
  await page.getByRole("button", { name: "Filtres" }).click();
  const sheet = page.getByRole("dialog");
  await sheet
    .getByLabel("Catégorie")
    .selectOption({ label: "Enfants et puériculture" });
  await sheet.getByText("10 km", { exact: true }).click();
  await sheet.getByText("Main propre", { exact: true }).click();
  await sheet.getByRole("button", { name: "Voir les résultats" }).click();

  await expect(page.getByText(proche)).toBeVisible();
  await expect(page.getByText(lointaine)).toHaveCount(0);

  // L'historique local retient la recherche.
  await page.getByLabel("Rechercher un objet").fill("");
  await expect(page.getByText("Tes dernières recherches")).toBeVisible();
  await expect(page.getByRole("button", { name: "pousette" })).toBeVisible();
});
