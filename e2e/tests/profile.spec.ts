import { expect, test, type Page } from "@playwright/test";

// F1.2 — Scénario Gherkin « dressing public » :
// les objets masqués ne sont jamais visibles d'un visiteur.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page): Promise<{ email: string; pseudo: string }> {
  const email = `vitrine-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `vitrine${Date.now() % 100000}`;
  await page.goto("/inscription");
  await page.getByLabel("Pseudo").fill(pseudo);
  await page.getByLabel("E-mail").fill(email);
  await page.getByLabel("Mot de passe", { exact: true }).fill("un-bon-mot-de-passe");
  await page.getByLabel("Code postal").fill("44000");
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
  return { email, pseudo };
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
  await page.getByLabel("Description").fill("Objet de test pour la vitrine publique.");
  await page.getByLabel("Valeur indicative (€)").fill("30");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("dressing public : les objets masqués sont invisibles, la ville s'affiche", async ({
  page,
  browser,
}) => {
  const { pseudo } = await verifiedUser(page);
  await publishItem(page, "Cube d'éveil");
  await publishItem(page, "Puzzle en bois");

  // Masquer le puzzle depuis le dressing.
  await page.goto("/dressing");
  await page.getByRole("button", { name: "Actions pour Puzzle en bois" }).click();
  await page.getByRole("button", { name: "Masquer", exact: true }).click();
  await expect(page.getByText("Masqué")).toBeVisible();

  // Bandeau propriétaire sur son propre profil public.
  await page.getByRole("link", { name: "Voir mon profil public" }).click();
  await expect(page.getByText("C'est ton profil vu par les autres.")).toBeVisible();

  // Un visiteur anonyme (nouveau contexte) voit 1 objet, la ville, jamais le masqué.
  const anonyme = await browser.newContext();
  const pageAnonyme = await anonyme.newPage();
  await pageAnonyme.goto(`/troqueur/${pseudo}`);
  await expect(pageAnonyme.getByRole("heading", { name: pseudo })).toBeVisible();
  await expect(pageAnonyme.getByText("Nantes")).toBeVisible();
  await expect(pageAnonyme.getByText("Nouveau troqueur")).toBeVisible();
  await expect(pageAnonyme.getByText("1 objet", { exact: true })).toBeVisible();
  await expect(pageAnonyme.getByText("Cube d'éveil")).toBeVisible();
  await expect(pageAnonyme.getByText("Puzzle en bois")).toHaveCount(0);
  await anonyme.close();
});

test("suppression d'objet depuis le dressing", async ({ page }) => {
  await verifiedUser(page);
  await publishItem(page, "Toupie à supprimer");

  await page.goto("/dressing");
  await page.getByRole("button", { name: "Actions pour Toupie à supprimer" }).click();
  await page.getByRole("button", { name: "Supprimer", exact: true }).click();
  await expect(page.getByText("supprimés pour de bon")).toBeVisible();
  await page.getByRole("button", { name: "Supprimer pour de bon" }).click();
  await expect(page.getByText("Objet supprimé.")).toBeVisible();
  await expect(page.getByText("Toupie à supprimer")).toHaveCount(0);
});
