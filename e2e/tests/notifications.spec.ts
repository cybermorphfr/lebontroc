import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F5.3 — la cloche : une proposition reçue allume le badge, le centre la
// liste, le clic mène au troc et éteint le badge.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `ntf-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `ntf${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  await page.getByLabel("Description").fill("Objet du test notifications.");
  await page.getByLabel("Valeur indicative (€)").fill("50");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("cloche : badge, centre, clic vers le troc", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const velo = `Draisienne${suffixe}`;
  const jeu = `Puzzle${suffixe}`;

  const contexteAlice: BrowserContext = await browser.newContext();
  const pageAlice = await contexteAlice.newPage();
  await verifiedUser(pageAlice, "44300");
  await publishItem(pageAlice, velo);

  await verifiedUser(page, "44000");
  await publishItem(page, jeu);
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill(velo);
  await page.getByText(velo).click();
  await page.getByRole("link", { name: "Proposer un troc" }).click();
  await page.getByRole("button", { name: `Choisir ${jeu}` }).click();
  await page.getByRole("button", { name: /Envoyer ma proposition/ }).click();
  await expect(page.getByRole("heading", { name: "Conversation" })).toBeVisible();

  // Alice voit le badge s'allumer, ouvre le centre, clique la notification.
  await pageAlice.goto("/");
  await expect(pageAlice.getByTestId("badge-notifications")).toHaveText("1");
  await pageAlice.getByRole("link", { name: /Notifications/ }).click();
  await expect(pageAlice.getByText("Nouvelle proposition de troc !")).toBeVisible();
  await pageAlice.getByText("Nouvelle proposition de troc !").click();
  await expect(pageAlice.getByRole("heading", { name: "Conversation" })).toBeVisible();

  // Retour : la notification est lue, le badge éteint.
  await pageAlice.goto("/notifications");
  await expect(pageAlice.getByTestId("badge-notifications")).toHaveCount(0);

  // Réglages : couper un e-mail et vérifier la persistance.
  await pageAlice.getByRole("link", { name: "Réglages" }).click();
  await expect(
    pageAlice.getByRole("heading", { name: "Notifications par e-mail" }),
  ).toBeVisible();
  await pageAlice.getByRole("switch").first().click();
  await expect(pageAlice.getByText("✓ Préférences enregistrées")).toBeVisible();
  await pageAlice.reload();
  await expect(pageAlice.getByRole("switch").first()).not.toBeChecked();
  await contexteAlice.close();
});
