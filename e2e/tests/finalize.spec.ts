import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F4.1 — Gherkin « finalisation croisée » : troc accepté main propre sans
// soulte, chacun saisit le code de l'autre → finalisé, objets troqués.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `fin-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `fin${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  await page.getByLabel("Description").fill("Objet de test pour la finalisation.");
  await page.getByLabel("Valeur indicative (€)").fill("50");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("finalisation croisée : codes échangés, troc finalisé, objets troqués", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const velo = `Trottinette${suffixe}`;
  const jeu = `Domino${suffixe}`;

  // Alice publie — son contexte reste ouvert jusqu'au bout.
  const contexteAlice: BrowserContext = await browser.newContext();
  const pageAlice = await contexteAlice.newPage();
  await verifiedUser(pageAlice, "44300");
  await publishItem(pageAlice, velo);

  // Bob publie et propose sans soulte.
  await verifiedUser(page, "44000");
  await publishItem(page, jeu);
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill(velo);
  await page.getByText(velo).click();
  await page.getByRole("link", { name: "Proposer un troc" }).click();
  await page.getByRole("button", { name: `Choisir ${jeu}` }).click();
  await page.getByRole("button", { name: /Envoyer ma proposition/ }).click();
  await expect(page.getByRole("heading", { name: "Conversation" })).toBeVisible();
  const urlTroc = page.url();

  // Alice accepte en main propre (l'envoi croisé existe depuis F4.3).
  await pageAlice.goto("/trocs");
  await pageAlice.getByRole("link", { name: /objet(s)? contre/ }).click();
  await pageAlice.getByRole("button", { name: "Accepter" }).click();
  await pageAlice.getByRole("button", { name: "En main propre" }).click();
  await expect(pageAlice.getByRole("heading", { name: "Organisez la remise" })).toBeVisible();

  // Chacun voit son code ; échange croisé.
  const codeAlice = await pageAlice.getByTestId("mon-code").innerText();
  await page.goto(urlTroc);
  await expect(page.getByRole("heading", { name: "Organisez la remise" })).toBeVisible();
  const codeBob = await page.getByTestId("mon-code").innerText();
  expect(codeAlice).toMatch(/^\d{6}$/);
  expect(codeBob).toMatch(/^\d{6}$/);

  // Un mauvais code est refusé proprement.
  await pageAlice.getByLabel("Code de l'autre partie").fill("000000");
  await pageAlice.getByRole("button", { name: "Confirmer la remise" }).click();
  await expect(pageAlice.getByText(/pas le bon code/)).toBeVisible();

  // Gherkin : chacun saisit le code de l'autre.
  await pageAlice.getByLabel("Code de l'autre partie").fill(codeBob);
  await pageAlice.getByRole("button", { name: "Confirmer la remise" }).click();
  await expect(pageAlice.getByText("Tu as confirmé la remise")).toBeVisible();

  await page.getByLabel("Code de l'autre partie").fill(codeAlice);
  await page.getByRole("button", { name: "Confirmer la remise" }).click();
  await expect(page.getByText("Troc finalisé !")).toBeVisible();

  // Les objets sortent des dressings publics et passent « Troqué » chez soi.
  await page.goto("/dressing");
  await expect(page.getByText("Troqué")).toBeVisible();
  await pageAlice.reload();
  await expect(pageAlice.getByText("Troc finalisé !")).toBeVisible();
  await contexteAlice.close();
});
