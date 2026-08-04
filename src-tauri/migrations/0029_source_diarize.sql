-- Per-recording speaker-label choice ("multiple people speaking" on the save
-- screen). NULL = follow the app default; 1/0 = the user's explicit choice for
-- this source, honored by the background transcription job even across an app
-- restart (the asr queue re-reads the row on resume).
ALTER TABLE sources ADD COLUMN diarize INTEGER;
