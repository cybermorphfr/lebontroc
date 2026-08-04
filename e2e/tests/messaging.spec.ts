import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F3.2 — Scénarios Gherkin « messagerie temps réel » : message visible sans
// recharger, coordonnées masquées avant acceptation, non-lus.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `msg-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `msg${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  await page.getByLabel("Description").fill("Objet de test pour la messagerie.");
  await page.getByLabel("Valeur indicative (€)").fill("40");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("temps réel, anti-contournement et non-lus", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const velo = `Tandem${suffixe}`;
  const jeu = `Ludo${suffixe}`;

  // Alice publie, son contexte reste ouvert.
  const contexteAlice: BrowserContext = await browser.newContext();
  const pageAlice = await contexteAlice.newPage();
  await verifiedUser(pageAlice, "44300");
  await publishItem(pageAlice, velo);

  // Bob publie et propose son jeu contre le tandem.
  await verifiedUser(page, "44000");
  await publishItem(page, jeu);
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill(velo);
  await page.getByText(velo).click();
  await page.getByRole("link", { name: "Proposer un troc" }).click();
  await page.getByRole("button", { name: `Choisir ${jeu}` }).click();
  await page.getByRole("button", { name: /Envoyer ma proposition/ }).click();
  await expect(page.getByRole("heading", { name: "Conversation" })).toBeVisible();

  // Anti-contournement : le numéro est masqué, avec un message pédagogique.
  await page.getByLabel("Ton message").fill("appelle-moi au 06 12 34 56 78");
  await page.getByRole("button", { name: "Envoyer" }).click();
  await expect(page.getByText("On a masqué des coordonnées")).toBeVisible();
  await expect(page.getByText(/appelle-moi au •/)).toBeVisible();
  const fil = await page.locator("main").innerText();
  expect(fil).not.toContain("06 12 34 56 78");

  // Non-lus : Alice voit un badge avant d'ouvrir.
  await pageAlice.goto("/trocs");
  await expect(pageAlice.getByLabel(/message(s)? non lu(s)?/)).toBeVisible();

  // Temps réel : les deux dans la conversation, sans rechargement.
  await pageAlice.getByRole("link", { name: /appelle-moi/ }).click();
  await expect(pageAlice.getByRole("heading", { name: "Conversation" })).toBeVisible();

  await pageAlice.getByLabel("Ton message").fill("Pas de téléphone, tout se passe ici !");
  await pageAlice.getByRole("button", { name: "Envoyer" }).click();
  // Bob voit le message d'Alice apparaître sans recharger la page.
  await expect(page.getByText("Pas de téléphone, tout se passe ici !")).toBeVisible({
    timeout: 10000,
  });

  // Et l'accusé de lecture d'Alice arrive chez Bob (« Lu »).
  await expect(page.getByText(/· Lu/).first()).toBeVisible({ timeout: 10000 });
  await contexteAlice.close();
});
