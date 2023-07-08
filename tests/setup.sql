DROP TABLE IF EXISTS accounts;
DROP TABLE IF EXISTS items;
DROP TABLE IF EXISTS users_books;
DROP TABLE IF EXISTS reviews;
DROP TABLE IF EXISTS addresses;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS books;


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


-- from https://launchschool.com/books/sql_first_edition/read/multi_tables

CREATE TABLE users (
  id serial,
  username VARCHAR(25) NOT NULL,
  enabled boolean DEFAULT TRUE,
  last_login timestamp NOT NULL DEFAULT NOW(),
  PRIMARY KEY (id)
);

/*
 one to one: User has one address
*/

CREATE TABLE addresses (
  user_id int NOT NULL,
  street VARCHAR(30) NOT NULL,
  city VARCHAR(30) NOT NULL,
  state VARCHAR(30) NOT NULL,
  PRIMARY KEY (user_id),
  CONSTRAINT fk_user_id FOREIGN KEY (user_id) REFERENCES users (id)
);

CREATE TABLE books (
  id serial,
  title VARCHAR(100) NOT NULL,
  author VARCHAR(100) NOT NULL,
  published_date timestamp NOT NULL,
  isbn int,
  PRIMARY KEY (id),
  UNIQUE (isbn)
);

/*
 one to many: Book has many reviews
*/

CREATE TABLE reviews (
  id serial,
  book_id int NOT NULL,
  user_id int NOT NULL,
  review_content VARCHAR(255),
  rating int,
  published_date timestamp DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE users_books (
  user_id int NOT NULL,
  book_id int NOT NULL,
  checkout_date timestamp,
  return_date timestamp,
  PRIMARY KEY (user_id, book_id),
  FOREIGN KEY (user_id) REFERENCES users(id) ON UPDATE CASCADE,
  FOREIGN KEY (book_id) REFERENCES books(id) ON UPDATE CASCADE
);

INSERT INTO accounts (username, password, email, created_on) values ('hoi', 'hoi', 'hoi@example.com', NOW());
INSERT INTO items (id, description) values ('ID-12345', 'This is a nice object');

-- from https://launchschool.com/books/sql_first_edition/read/joins

-- A user
INSERT INTO users (id, username) VALUES (1, 'John Smith');

-- An address
INSERT INTO addresses (user_id, street, city, state)
VALUES (1, '1 Market Street', 'San Francisco', 'CA');

-- A book
INSERT INTO books (id, title, author, published_date)
VALUES(1, 'My First SQL book', 'Mary Parker', NOW());

INSERT INTO users_books (user_id, book_id, checkout_date)
VALUES(1, 1, NOW());

-- A review
INSERT INTO reviews (id, book_id, user_id, review_content)
VALUES (1, 1, 1, 'My first review');

-- A second book
INSERT INTO books (id, title, author, published_date)
VALUES(2, 'My Second SQL book','John Mayer', NOW());

INSERT INTO users_books (user_id, book_id, checkout_date)
VALUES (1, 2 ,NOW());

-- A second review
INSERT INTO reviews (id, book_id, user_id, review_content)
VALUES (2, 2, 1, 'My second review');

-- A second User
INSERT INTO users (id, username) VALUES (2, 'Jane Smiley');

-- A second address
INSERT INTO addresses (user_id, street, city, state)
VALUES (2, '2 Elm Street', 'San Francisco', 'CA');

INSERT INTO users_books (user_id, book_id, checkout_date)
VALUES(2, 2 , NOW());

-- A third review
INSERT INTO reviews (id, book_id, user_id, review_content )
VALUES (3, 2, 2, 'review_content');

-- A Third User
INSERT INTO users (id, username) VALUES (3, 'Alice Munro');

-- A Third book
INSERT INTO books (id, title, author, published_date)
VALUES(3, 'My Third SQL book','Cary Flint', NOW());
