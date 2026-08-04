import { expect, test, type Browser, type Page } from "@playwright/test";

// F2.1 — Scénarios Gherkin « fil d'accueil et fiche objet » :
// le fil privilégie le local ; la fiche montre ville et distance, jamais
// d'adresse ni de code postal complet.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(
  page: Page,
  postalCode: string,
): Promise<{ email: string; pseudo: string }> {
  const email = `fil-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `fil${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  await page.getByLabel("Description").fill("Objet de test pour le fil d'accueil.");
  await page.getByLabel("Valeur indicative (€)").fill("30");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

/** Inscrit un utilisateur dans un contexte jetable et publie un objet. */
async function publishAs(browser: Browser, postalCode: string, title: string) {
  const context = await browser.newContext();
  const page = await context.newPage();
  await verifiedUser(page, postalCode);
  await publishItem(page, title);
  await context.close();
}

test("le fil privilégie le local, la fiche montre la ville sans jamais l'adresse", async ({
  page,
  browser,
}) => {
  // L'objet proche est publié AVANT le lointain : si le proche sort quand même
  // en premier, c'est bien la distance qui l'emporte sur la récence.
  await publishAs(browser, "44300", "Théière proche");
  await publishAs(browser, "75001", "Théière lointaine");

  await verifiedUser(page, "44000");
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Autour de toi" })).toBeVisible();

  const cards = page.locator("main ul li");
  await expect(cards.filter({ hasText: "Théière proche" }).first()).toBeVisible();
  const proche = await cards.filter({ hasText: "Théière proche" }).first().boundingBox();
  const lointaine = await cards.filter({ hasText: "Théière lointaine" }).first().boundingBox();
  if (!proche || !lointaine) throw new Error("cartes introuvables");
  // La grille remplit ligne par ligne : plus haut ou plus à gauche = mieux classé.
  expect(
    proche.y < lointaine.y || (proche.y === lointaine.y && proche.x < lointaine.x),
  ).toBe(true);

  // Fiche objet : ville, distance, CTA inactif, jamais le code postal.
  await cards.filter({ hasText: "Théière lointaine" }).first().getByRole("link").click();
  await expect(page.getByRole("heading", { name: "Théière lointaine" })).toBeVisible();
  await expect(page.getByText("Paris", { exact: false }).first()).toBeVisible();
  await expect(page.getByText(/à \d+ km/).first()).toBeVisible();
  const troc = page.getByRole("button", { name: "Proposer un troc" });
  await expect(troc).toBeVisible();
  await expect(troc).toBeDisabled();
  const contenu = await page.locator("main").innerText();
  expect(contenu).not.toContain("75001");

  // Galerie plein écran.
  await page.getByRole("button", { name: "Voir les photos en plein écran" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "Fermer" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);

  // Encart propriétaire → dressing public.
  await page.getByRole("link", { name: /Voir son dressing/ }).click();
  await expect(page.getByText("Théière lointaine")).toBeVisible();
});
