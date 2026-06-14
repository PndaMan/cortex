-- Store the ORIGINAL framework file (not just extracted text) so it can be
-- viewed in-app as a PDF, exactly like a source. `file_path` is the persisted
-- viewable file (the original PDF/image, or a LibreOffice-rendered PDF for
-- docx/pptx); `view_kind` is how the UI renders it: pdf | image | text.
ALTER TABLE subject_frameworks ADD COLUMN file_path TEXT;
ALTER TABLE subject_frameworks ADD COLUMN view_kind TEXT;
