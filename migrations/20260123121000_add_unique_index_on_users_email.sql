-- Remove duplicate users keeping the earliest id for each email
DELETE u1 FROM users u1
INNER JOIN users u2
    ON u1.email = u2.email
    AND u1.id > u2.id;

-- Add unique index on email to prevent future duplicates
ALTER TABLE users ADD UNIQUE INDEX ux_users_email (email);
