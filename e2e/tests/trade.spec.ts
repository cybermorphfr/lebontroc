import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F3.1 — Scénarios Gherkin « proposition de troc » : composeur multi-objets
// avec soulte, plafond à 50 % du meilleur objet, vue puis refus.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `troc-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `troc${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  await page.getByLabel("Description").fill("Objet de test pour le troc.");
  await page.getByLabel("Valeur indicative (€)").fill(valueEuros);
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

test("composer, plafond de soulte, vue et refus", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const velo = `Vélo Vintage${suffixe}`;
  const console_ = `Console Rétro${suffixe}`;

  // Alice publie son vélo (150 €) — son contexte reste ouvert.
  const contexteAlice: BrowserContext = await browser.newContext();
  const pageAlice = await contexteAlice.newPage();
  await verifiedUser(pageAlice, "44300");
  await publishItem(pageAlice, velo, "150");

  // Bob publie sa console (120 €) puis ouvre le composeur depuis la fiche du vélo.
  await verifiedUser(page, "44000");
  await publishItem(page, console_, "120");
  await page.goto("/recherche");
  await page.getByLabel("Rechercher un objet").fill(velo);
  await page.getByText(velo).click();
  await page.getByRole("link", { name: "Proposer un troc" }).click();
  await expect(page.getByRole("heading", { name: "Proposer un troc" })).toBeVisible();

  // Le vélo est présélectionné côté « Tu reçois » ; Bob choisit sa console.
  await expect(page.getByRole("button", { name: `Retirer ${velo}` })).toBeVisible();
  await page.getByRole("button", { name: `Choisir ${console_}` }).click();

  // Soulte : plafond affiché à 75 € (50 % du vélo à 150 €), curseur borné.
  await page.getByText("J'ajoute des euros", { exact: true }).click();
  await expect(page.getByText(/Plafond : 75 €/)).toBeVisible();
  const curseur = page.getByLabel("Montant de la soulte");
  await expect(curseur).toHaveAttribute("max", "75");
  await curseur.fill("30");
  await expect(page.getByText("30 €", { exact: true })).toBeVisible();

  // Envoi → détail « Envoyée » avec le récap.
  await page.getByRole("button", { name: /Envoyer ma proposition/ }).click();
  await expect(page.getByTestId("statut-proposition")).toHaveText("Envoyée");
  await expect(page.getByRole("link", { name: velo })).toBeVisible();
  await expect(page.getByRole("link", { name: console_ })).toBeVisible();
  await expect(page.getByText("+ 30 € de soulte")).toBeVisible();

  // Alice ouvre sa boîte : la proposition passe à « Vue », puis elle refuse.
  await pageAlice.goto("/trocs");
  await pageAlice.getByRole("link", { name: /1 objet contre 1/ }).click();
  await expect(pageAlice.getByTestId("statut-proposition")).toHaveText("Vue");
  await pageAlice.getByRole("button", { name: "Refuser la proposition" }).click();
  await pageAlice.getByRole("button", { name: "Oui, je refuse" }).click();
  await expect(pageAlice.getByTestId("statut-proposition")).toHaveText("Refusée");
  await contexteAlice.close();

  // Côté Bob : le refus est visible dans ses envoyées.
  await page.goto("/trocs?box=envoyees");
  await expect(page.getByText("Refusée")).toBeVisible();
});
