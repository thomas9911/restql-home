DROP TABLE IF EXISTS accounts;
DROP TABLE IF EXISTS items;

CREATE TABLE accounts (
        id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
        username VARCHAR ( 50 ) UNIQUE NOT NULL,
        password VARCHAR ( 50 ) NOT NULL,
        email VARCHAR ( 255 ) UNIQUE NOT NULL,
        created_on TIMESTAMP NOT NULL,
        last_login TIMESTAMP,
        created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE items (
        id VARCHAR (64) PRIMARY KEY,
        description TEXT NOT NULL,
        created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO accounts (username, password, email, created_on) values ('hoi', 'hoi', 'hoi@example.com', NOW());
INSERT INTO items (id, description) values ('ID-12345', 'This is a nice object');
