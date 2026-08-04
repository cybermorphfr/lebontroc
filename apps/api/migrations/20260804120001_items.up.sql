-- F1.1 — catalogue : catégories (arbre 3 niveaux, seed 2 niveaux), objets,
-- photos et registre anti-orphelins des uploads présignés.

CREATE TABLE categories (
    id              SMALLINT PRIMARY KEY,
    parent_id       SMALLINT REFERENCES categories(id),
    slug            TEXT NOT NULL UNIQUE,
    label           TEXT NOT NULL,
    icon            TEXT,
    depth           SMALLINT NOT NULL CHECK (depth BETWEEN 1 AND 3),
    sort_order      SMALLINT NOT NULL DEFAULT 0,
    value_min_cents INTEGER CHECK (value_min_cents > 0),
    value_max_cents INTEGER CHECK (value_max_cents >= value_min_cents)
);

CREATE TABLE items (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title           TEXT NOT NULL CHECK (char_length(title) BETWEEN 3 AND 80),
    description     TEXT NOT NULL CHECK (char_length(description) BETWEEN 10 AND 2000),
    category_id     SMALLINT NOT NULL REFERENCES categories(id),
    condition       TEXT NOT NULL
                    CHECK (condition IN ('neuf','tres_bon_etat','bon_etat','correct')),
    status          TEXT NOT NULL DEFAULT 'disponible'
                    CHECK (status IN ('disponible','reserve','troque','masque')),
    -- Plafond produit 2 000 € : borne le futur plafond de soulte (50 %).
    value_cents     INTEGER NOT NULL CHECK (value_cents BETWEEN 100 AND 200000),
    delivery_pref   TEXT NOT NULL CHECK (delivery_pref IN ('main_propre','envoi','les_deux')),
    exchange_wishes TEXT CHECK (char_length(exchange_wishes) <= 300),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX items_owner_idx    ON items (owner_id, created_at DESC);
CREATE INDEX items_feed_idx     ON items (status, created_at DESC);
CREATE INDEX items_category_idx ON items (category_id);

-- Uploads présignés en attente de rattachement à un objet (purge à 24 h).
CREATE TABLE photo_uploads (
    photo_id     UUID PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    s3_key       TEXT NOT NULL,
    content_type TEXT NOT NULL CHECK (content_type IN ('image/webp','image/jpeg')),
    byte_size    INTEGER NOT NULL CHECK (byte_size > 0 AND byte_size <= 5242880),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX photo_uploads_created_idx ON photo_uploads (created_at);

CREATE TABLE item_photos (
    photo_id     UUID PRIMARY KEY,
    item_id      UUID NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    position     SMALLINT NOT NULL CHECK (position BETWEEN 0 AND 7),
    s3_key       TEXT NOT NULL,
    content_type TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- DEFERRABLE : le réordonnancement réécrit les positions en transaction.
    UNIQUE (item_id, position) DEFERRABLE INITIALLY DEFERRED
);

-- ————— Seed des catégories (référentiel produit v1) —————
-- Racines : id 1–9. Sous-catégories : id = racine × 10 + n.
INSERT INTO categories (id, parent_id, slug, label, icon, depth, sort_order, value_min_cents, value_max_cents) VALUES
    (1, NULL, 'electronique',     'Électronique',              'smartphone', 1, 1, 500, 80000),
    (2, NULL, 'mode',             'Vêtements et accessoires',  'shirt',      1, 2, 200, 20000),
    (3, NULL, 'enfants',          'Enfants et puériculture',   'baby',       1, 3, 200, 25000),
    (4, NULL, 'maison',           'Maison et déco',            'armchair',   1, 4, 500, 40000),
    (5, NULL, 'jardin-bricolage', 'Jardin et bricolage',       'hammer',     1, 5, 500, 30000),
    (6, NULL, 'sport',            'Sport et plein air',        'bike',       1, 6, 500, 50000),
    (7, NULL, 'culture',          'Livres, musique et films',  'book-open',  1, 7, 100, 15000),
    (8, NULL, 'jeux-loisirs',     'Jeux et loisirs créatifs',  'puzzle',     1, 8, 100, 15000),
    (9, NULL, 'autres',           'Autres objets',             'package',    1, 9, 100, 20000);

INSERT INTO categories (id, parent_id, slug, label, depth, sort_order) VALUES
    (11, 1, 'smartphones',          'Smartphones et téléphones',      2, 1),
    (12, 1, 'informatique',         'Ordinateurs et informatique',    2, 2),
    (13, 1, 'tablettes-liseuses',   'Tablettes et liseuses',          2, 3),
    (14, 1, 'photo-video',          'Photo et vidéo',                 2, 4),
    (15, 1, 'audio',                'Son et audio',                   2, 5),
    (16, 1, 'tv-video',             'TV et vidéoprojection',          2, 6),
    (17, 1, 'consoles-jeux-video',  'Consoles et jeux vidéo',         2, 7),
    (21, 2, 'femme',                'Femme',                          2, 1),
    (22, 2, 'homme',                'Homme',                          2, 2),
    (23, 2, 'chaussures',           'Chaussures',                     2, 3),
    (24, 2, 'sacs-bagages',         'Sacs et bagages',                2, 4),
    (25, 2, 'bijoux-montres',       'Bijoux et montres',              2, 5),
    (26, 2, 'accessoires-mode',     'Accessoires',                    2, 6),
    (31, 3, 'poussettes-portage',   'Poussettes et portage',          2, 1),
    (32, 3, 'sieges-auto',          'Sièges auto',                    2, 2),
    (33, 3, 'materiel-puericulture','Matériel et mobilier bébé',      2, 3),
    (34, 3, 'vetements-enfant',     'Vêtements bébé et enfant',       2, 4),
    (35, 3, 'jouets',               'Jouets et éveil',                2, 5),
    (36, 3, 'livres-jeunesse',      'Livres jeunesse',                2, 6),
    (41, 4, 'meubles',              'Meubles',                        2, 1),
    (42, 4, 'deco-luminaires',      'Déco et luminaires',             2, 2),
    (43, 4, 'electromenager',       'Électroménager',                 2, 3),
    (44, 4, 'cuisine-arts-table',   'Cuisine et arts de la table',    2, 4),
    (45, 4, 'linge-maison',         'Linge de maison',                2, 5),
    (46, 4, 'rangement',            'Rangement',                      2, 6),
    (51, 5, 'outillage',            'Outillage',                      2, 1),
    (52, 5, 'jardinage',            'Jardinage',                      2, 2),
    (53, 5, 'mobilier-jardin',      'Mobilier de jardin',             2, 3),
    (54, 5, 'plantes',              'Plantes',                        2, 4),
    (55, 5, 'materiaux',            'Matériaux et quincaillerie',     2, 5),
    (61, 6, 'velos-mobilite',       'Vélos et mobilité',              2, 1),
    (62, 6, 'fitness',              'Fitness et musculation',         2, 2),
    (63, 6, 'rando-camping',        'Randonnée et camping',           2, 3),
    (64, 6, 'sports-hiver',         'Sports d''hiver et glisse',      2, 4),
    (65, 6, 'sports-eau',           'Sports d''eau',                  2, 5),
    (66, 6, 'sports-raquette',      'Sports de raquette',             2, 6),
    (67, 6, 'sports-collectifs',    'Sports collectifs',              2, 7),
    (71, 7, 'livres-bd',            'Livres, BD et mangas',           2, 1),
    (72, 7, 'musique',              'CD, vinyles et musique',         2, 2),
    (73, 7, 'films-series',         'DVD et Blu-ray',                 2, 3),
    (74, 7, 'instruments',          'Instruments de musique',         2, 4),
    (81, 8, 'jeux-societe',         'Jeux de société et puzzles',     2, 1),
    (82, 8, 'loisirs-creatifs',     'Loisirs créatifs et mercerie',   2, 2),
    (83, 8, 'modelisme',            'Modélisme et maquettes',         2, 3),
    (84, 8, 'collection',           'Collection (cartes, figurines)', 2, 4),
    (85, 8, 'jeux-exterieur',       'Jeux d''extérieur',              2, 5),
    (91, 9, 'accessoires-animaux',  'Accessoires pour animaux',       2, 1),
    (92, 9, 'accessoires-auto-moto','Accessoires auto et moto',       2, 2),
    (93, 9, 'beaute-bien-etre',     'Beauté et bien-être',            2, 3),
    (94, 9, 'bureau-papeterie',     'Bureau et papeterie',            2, 4),
    (95, 9, 'divers',               'Divers',                         2, 5);
