import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F4.2 — Gherkin « séquestre puis libération » : troc accepté avec soulte,
// le payeur préautorise (PSP simulé), la remise croisée capture le paiement.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `pay-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `pay${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  await page.getByLabel("Description").fill("Objet de test pour la soulte.");
  await page.getByLabel("Valeur indicative (€)").fill("50");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("soulte séquestrée : refus simulé, préautorisation, capture à la remise", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const velo = `Draisienne${suffixe}`;
  const jeu = `Meccano${suffixe}`;

  // Alice publie — elle recevra la soulte.
  const contexteAlice: BrowserContext = await browser.newContext();
  const pageAlice = await contexteAlice.newPage();
  await verifiedUser(pageAlice, "44300");
  await publishItem(pageAlice, velo);

  // Bob publie et propose SA soulte : jeu + 10 € contre le vélo.
  await verifiedUser(page, "44000");
  await publishItem(page, jeu);
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill(velo);
  await page.getByText(velo).click();
  await page.getByRole("link", { name: "Proposer un troc" }).click();
  await page.getByRole("button", { name: `Choisir ${jeu}` }).click();
  await page.getByText("J'ajoute des euros", { exact: true }).click();
  await page.getByLabel("Montant de la soulte").fill("10");
  await page.getByRole("button", { name: /Envoyer ma proposition/ }).click();
  await expect(page.getByPlaceholder("Envoyer un message")).toBeVisible();
  const urlTroc = page.url();

  // Alice accepte en main propre : le troc attend le paiement de Bob.
  await pageAlice.goto("/trocs");
  await pageAlice.getByRole("link", { name: /objet(s)? contre/ }).click();
  await pageAlice.getByRole("button", { name: "Accepter", exact: true }).click();
  await pageAlice.getByRole("button", { name: "En main propre" }).click();
  await expect(
    pageAlice.getByRole("heading", { name: /soulte en cours de règlement/ }),
  ).toBeVisible();

  // Bob voit l'écran de paiement ; la carte magique 0002 est refusée.
  await page.goto(urlTroc);
  await expect(page.getByRole("heading", { name: /Sécurise la soulte de 10 €/ })).toBeVisible();
  await page.getByLabel("Numéro de carte").fill("4970 0000 0000 0002");
  await page.getByRole("button", { name: /Bloquer 10 €/ }).click();
  await expect(page.getByText(/refusé/)).toBeVisible();

  // La bonne carte séquestre : l'écran de remise apparaît, soulte affichée.
  await page.getByLabel("Numéro de carte").fill("4970 0000 0000 0000");
  await page.getByRole("button", { name: /Bloquer 10 €/ }).click();
  await expect(page.getByRole("heading", { name: "Organisez la remise" })).toBeVisible();
  await expect(page.getByText(/Soulte de 10 € sécurisée/)).toBeVisible();

  // Alice voit l'écran basculer (temps réel ou rechargement de secours).
  await pageAlice.reload();
  await expect(pageAlice.getByRole("heading", { name: "Organisez la remise" })).toBeVisible();
  await expect(pageAlice.getByText(/te sera transférée à la remise/)).toBeVisible();

  // Échange croisé des codes → finalisé, la soulte est transférée.
  const codeAlice = await pageAlice.getByTestId("mon-code").innerText();
  const codeBob = await page.getByTestId("mon-code").innerText();
  await pageAlice.getByLabel("Code de l'autre partie").fill(codeBob);
  await pageAlice.getByRole("button", { name: "Confirmer la remise" }).click();
  await expect(pageAlice.getByText("Tu as confirmé la remise")).toBeVisible();
  await page.getByLabel("Code de l'autre partie").fill(codeAlice);
  await page.getByRole("button", { name: "Confirmer la remise" }).click();
  await expect(page.getByText("Troc finalisé !")).toBeVisible();
  // F5.2 : la capture attend la fenêtre de contestation de 48 h.
  await expect(page.getByText(/seront débités sous 48 h/)).toBeVisible();
  await pageAlice.reload();
  await expect(pageAlice.getByText(/te seront transférés sous 48 h/)).toBeVisible();
  await contexteAlice.close();
});
