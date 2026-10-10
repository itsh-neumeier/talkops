-- A number routed to an extension may ring further extensions at the same
-- time; unanswered calls go to the voicemail of destination_id.
ALTER TABLE numbers ADD COLUMN extra_extensions UUID[] NOT NULL DEFAULT '{}';
