-- FSRS scheduler state. NULL on legacy SM-2 rows — they're seeded from their
-- existing interval/ease the first time they're graded under FSRS.
ALTER TABLE srs_cards ADD COLUMN stability REAL;
ALTER TABLE srs_cards ADD COLUMN difficulty REAL;
