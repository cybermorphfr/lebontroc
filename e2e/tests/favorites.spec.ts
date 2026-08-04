import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F2.3 — Scénario Gherkin « favori conservé » : le cœur survit au retour,
// le propriétaire voit son compteur (sans savoir qui) ; liste d'envies.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `coeur-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `coeur${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  return { pseudo };
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
  await page.getByLabel("Description").fill("Objet de test pour les favoris.");
  await page.getByLabel("Valeur indicative (€)").fill("30");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("favori conservé, compteur du propriétaire et liste d'envies", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const titre = `Mobile Marin${Date.now() % 100000}`;

  // Le propriétaire publie — son contexte reste ouvert pour la fin du test.
  const contexteProprio: BrowserContext = await browser.newContext();
  const pageProprio = await contexteProprio.newPage();
  const proprio = await verifiedUser(pageProprio, "44300");
  await publishItem(pageProprio, titre);

  // Le fan trouve l'objet, ouvre la fiche et pose un cœur.
  await verifiedUser(page, "44000");
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill(titre);
  await page.getByText(titre).click();
  await expect(page.getByRole("heading", { name: titre })).toBeVisible();
  await page.getByRole("button", { name: "Ajouter aux favoris" }).click();
  await expect(page.getByRole("button", { name: "Retirer des favoris" })).toBeVisible();

  // Gherkin : je reviens → l'objet est dans ma page favoris.
  await page.goto("/favoris");
  await expect(page.getByText(titre)).toBeVisible();
  await page.reload();
  await expect(page.getByText(titre)).toBeVisible();

  // Le propriétaire voit « 1 favori » sur sa fiche, sans savoir qui.
  await pageProprio.goto(`/troqueur/${encodeURIComponent(proprio.pseudo)}`);
  await pageProprio.getByRole("link", { name: new RegExp(titre) }).click();
  await expect(pageProprio.getByText(/1 favori/)).toBeVisible();
  // Et le compteur apparaît dans son dressing.
  await pageProprio.goto("/dressing");
  await expect(pageProprio.getByText("♥ 1")).toBeVisible();
  await contexteProprio.close();

  // Liste d'envies : remplie dans le profil, conservée au rechargement.
  await page.goto("/profil");
  await page.getByLabel("Catégorie de l'envie 1").selectOption({
    label: "Enfants et puériculture",
  });
  await page.getByLabel("Mots-clés de l'envie 1").fill("poussette yoyo");
  await page.getByRole("button", { name: "Enregistrer mes envies" }).click();
  await expect(page.getByText("C'est noté !")).toBeVisible();
  await page.reload();
  await expect(page.getByLabel("Mots-clés de l'envie 1")).toHaveValue("poussette yoyo");
  await expect(page.getByLabel("Catégorie de l'envie 1")).toHaveValue(/\d+/);
});
