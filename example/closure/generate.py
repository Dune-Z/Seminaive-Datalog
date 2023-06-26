import os
import sqlite3
import random
NUM_NODES = 500
NUM_EDGES = 1000


def main():
    path = __file__.split('/')[: -1]
    path = '/'.join(path) + '/closure.db'
    if os.path.exists(path):
        os.remove(path)
    conn = sqlite3.connect(path)
    cursor = conn.cursor()
    cursor.execute("\
        CREATE TABLE IF NOT EXISTS edge (\
            column_0 TEXT NOT NULL,\
            column_1 TEXT NOT NULL\
        )\
    ")
    for _ in range(NUM_EDGES):
        src = random.randint(0, NUM_NODES - 1)
        tgt = random.randint(0, NUM_NODES - 1)
        cursor.execute("INSERT INTO edge VALUES (?, ?)", (src, tgt))
    conn.commit()
    conn.close()


if __name__ == '__main__':
    main()
