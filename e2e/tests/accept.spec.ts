import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F3.3 — Gherkin « acceptation » : contre-proposition puis troc conclu,
// objets réservés. (La course concurrente est couverte par un test SQL.)

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `acc-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `acc${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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

async function publishItem(page: Page, title: string, valueEuros: string) {
  await page.goto("/publier");
  await page.locator('input[type="file"]').setInputFiles([
    { name: "photo.png", mimeType: "image/png", buffer: TINY_PNG },
  ]);
  await page.getByLabel("Titre").fill(title);
  await page.getByRole("button", { name: "Choisis une catégorie" }).click();
  await page.getByRole("button", { name: "Enfants et puériculture" }).click();
  await page.getByRole("button", { name: "Jouets et éveil" }).click();
  await page.getByLabel("Description").fill("Objet de test pour l'acceptation.");
  await page.getByLabel("Valeur indicative (€)").fill(valueEuros);
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("contre-proposition puis acceptation : troc conclu, objets réservés", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const velo = `Draisienne${suffixe}`;
  const jeu = `Mécano${suffixe}`;

  // Alice publie sa draisienne — son contexte reste ouvert.
  const contexteAlice: BrowserContext = await browser.newContext();
  const pageAlice = await contexteAlice.newPage();
  await verifiedUser(pageAlice, "44300");
  await publishItem(pageAlice, velo, "150");

  // Bob publie son jeu et propose l'échange sans soulte.
  await verifiedUser(page, "44000");
  await publishItem(page, jeu, "120");
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill(velo);
  await page.getByText(velo).click();
  await page.getByRole("link", { name: "Proposer un troc" }).click();
  await page.getByRole("button", { name: `Choisir ${jeu}` }).click();
  await page.getByRole("button", { name: /Envoyer ma proposition/ }).click();
  await expect(page.getByPlaceholder("Envoyer un message")).toBeVisible();

  // Alice contre-propose (sans soulte — pendant la bêta, seuls les trocs
  // sans soulte se concluent ; la garde est couverte par les tests SQL).
  await pageAlice.goto("/trocs");
  await pageAlice.getByRole("link", { name: /objet(s)? contre/ }).click();
  await pageAlice.getByRole("link", { name: "Contre-proposer" }).click();
  await expect(
    pageAlice.getByRole("heading", { name: "Contre-proposer", level: 1 }),
  ).toBeVisible();
  // La composition initiale est préremplie (draisienne offerte, jeu demandé).
  await expect(pageAlice.getByRole("button", { name: `Retirer ${velo}` })).toBeVisible();
  await expect(pageAlice.getByRole("button", { name: `Retirer ${jeu}` })).toBeVisible();
  await pageAlice.getByRole("button", { name: /Envoyer ma contre-proposition/ }).click();
  await expect(pageAlice.getByText("Envoyée")).toBeVisible();

  // Bob voit la contre-proposition et l'accepte en main propre.
  await page.goto("/trocs");
  await page.getByRole("link", { name: /De acc/ }).click();
  await page.getByRole("button", { name: "Accepter" }).click();
  await page.getByRole("button", { name: "En main propre" }).click();
  await expect(page.getByText(/Troc conclu/)).toBeVisible();
  await expect(page.getByText("Acceptée")).toBeVisible();

  // Les objets sont réservés dans les deux dressings.
  await page.goto("/dressing");
  await expect(page.getByText("Réservé")).toBeVisible();
  await pageAlice.goto("/dressing");
  await expect(pageAlice.getByText("Réservé")).toBeVisible();
  await contexteAlice.close();
});
