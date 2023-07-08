#! /bin/bash

# curl http://localhost:9503/accounts/89592c86-f85d-4527-bdb9-4c3f5dd63f2d

# curl -X POST --data '{"email": "example@example.com", "username": "example", "password": "example", "created_on": "2020-04-12T12:23:34"}' --header 'Content-Type: application/json' http://localhost:9503/accounts


curl http://localhost:9503/accounts?select=myid:id,username
