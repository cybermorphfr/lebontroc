import { expect, test, type Browser, type BrowserContext, type Page } from "@playwright/test";

// F4.3 — Gherkin « envoi croisé nominal » : troc accepté par envoi, chacun
// configure son colis, paie ses frais (PSP simulé), dépose, retire et
// confirme — le troc se finalise sans rendez-vous.

const MAILPIT = process.env.MAILPIT_URL ?? "http://localhost:8025";

const TINY_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function verifiedUser(page: Page, postalCode: string): Promise<{ pseudo: string }> {
  const email = `shp-${Date.now()}-${Math.floor(Math.random() * 10000)}@exemple.fr`;
  const pseudo = `shp${Date.now() % 100000}${Math.floor(Math.random() * 100)}`;
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
  await page.getByLabel("Description").fill("Objet de test pour l'envoi croisé.");
  await page.getByLabel("Valeur indicative (€)").fill("50");
  await page.getByRole("button", { name: "Publier", exact: true }).click();
  await expect(page.getByRole("heading", { name: "C'est publié !" })).toBeVisible();
}

/**
 * Configure format S + premier relais, puis paie (carte simulée OK).
 * Le premier payeur voit « règlement sécurisé » ; le second active le troc
 * et arrive directement sur « Vos colis ».
 */
async function setupAndPay(page: Page, headingAfter: RegExp) {
  await expect(page.getByRole("heading", { name: "Prépare ton envoi" })).toBeVisible();
  await page.getByRole("radio", { name: /S — jusqu'à 1 kg/ }).check();
  await page.getByLabel("Ton point relais de réception").selectOption({ index: 1 });
  await page.getByRole("button", { name: "Valider mon envoi" }).click();
  await expect(page.getByText("Total à bloquer")).toBeVisible();
  await page.getByRole("button", { name: /Bloquer 6,50 €/ }).click();
  await expect(page.getByRole("heading", { name: headingAfter })).toBeVisible();
}

test("envoi croisé : config, frais, étiquettes, dépôts et double confirmation", async ({
  page,
  browser,
}: {
  page: Page;
  browser: Browser;
}) => {
  const suffixe = `${Date.now() % 100000}`;
  const velo = `Patinette${suffixe}`;
  const jeu = `Kapla${suffixe}`;

  // Alice publie — elle enverra son vélo, recevra le jeu.
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
  await expect(page.getByPlaceholder("Envoyer un message")).toBeVisible();
  const urlTroc = page.url();

  // Alice accepte PAR ENVOI, configure son colis et paie 6,50 € (S + service).
  await pageAlice.goto("/trocs");
  await pageAlice.getByRole("link", { name: /objet(s)? contre/ }).click();
  await pageAlice.getByRole("button", { name: "Accepter" }).click();
  await pageAlice.getByRole("button", { name: "Par envoi (point relais)" }).click();
  await setupAndPay(pageAlice, /Ton règlement est sécurisé/);

  // Bob fait pareil de son côté : le troc s'active, les étiquettes tombent.
  await page.goto(urlTroc);
  await setupAndPay(page, /Vos colis/);
  const codeDepotBob = await page.getByTestId("code-depot").innerText();
  expect(codeDepotBob).toMatch(/^LBT\d{8}$/);

  // Bob dépose ; le simulateur fait arriver le colis directement chez Alice.
  await page.getByRole("button", { name: "J'ai déposé mon colis" }).click();
  await expect(page.getByText("Arrivé au point relais", { exact: false })).toBeVisible();
  await pageAlice.reload();
  await expect(pageAlice.getByText("Arrivé au point relais", { exact: false })).toBeVisible();

  // Alice récupère, confirme, et expédie le sien.
  await pageAlice.getByRole("button", { name: "Je l'ai récupéré" }).click();
  await pageAlice.getByRole("button", { name: "Tout est OK ✓" }).click();
  await expect(pageAlice.getByText("Tu as confirmé la réception")).toBeVisible();
  await pageAlice.getByRole("button", { name: "J'ai déposé mon colis" }).click();

  // Bob réceptionne et confirme : troc finalisé des deux côtés.
  await page.reload();
  await page.getByRole("button", { name: "Je l'ai récupéré" }).click();
  await page.getByRole("button", { name: "Tout est OK ✓" }).click();
  await expect(page.getByText("Troc finalisé !")).toBeVisible();
  await pageAlice.reload();
  await expect(pageAlice.getByText("Troc finalisé !")).toBeVisible();
  await contexteAlice.close();
});
