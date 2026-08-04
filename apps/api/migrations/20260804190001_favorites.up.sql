-- F2.3 — favoris (cœur sur un objet) + liste d'envies légère (3 lignes
-- « ce que je cherche » : catégorie + mots-clés). Aucun algorithme au MVP.
CREATE TABLE favorites (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id    UUID NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, item_id)
);
CREATE INDEX favorites_item_idx ON favorites (item_id);

CREATE TABLE wishlist_entries (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    position    SMALLINT NOT NULL CHECK (position BETWEEN 0 AND 2),
    category_id SMALLINT REFERENCES categories(id),
    keywords    TEXT NOT NULL DEFAULT '' CHECK (char_length(keywords) <= 120),
    PRIMARY KEY (user_id, position)
);
