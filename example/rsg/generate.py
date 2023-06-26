import os
import sqlite3


def main():
    path = __file__.split('/')[: -1]
    path = '/'.join(path) + '/rsg.db'
    if os.path.exists(path):
        os.remove(path)
    conn = sqlite3.connect(path)
    cursor = conn.cursor()
    cursor.execute("\
        CREATE TABLE IF NOT EXISTS up (\
            column_0 TEXT NOT NULL,\
            column_1 TEXT NOT NULL\
        )\
    ")
    up = [
        ('a', 'e'),
        ('a', 'f'),
        ('f', 'm'),
        ('g', 'n'),
        ('h', 'n'),
        ('i', 'o'),
        ('j', 'o'),
    ]
    for src, tgt in up:
        cursor.execute("INSERT INTO up VALUES (?, ?)", (src, tgt))
    cursor.execute("\
        CREATE TABLE IF NOT EXISTS flat (\
            column_0 TEXT NOT NULL,\
            column_1 TEXT NOT NULL\
        )\
    ")
    flat = [
        ('g', 'f'),
        ('m', 'n'),
        ('m', 'o'),
        ('p', 'm'),
    ]
    for src, tgt in flat:
        cursor.execute("INSERT INTO flat VALUES (?, ?)", (src, tgt))
    cursor.execute("\
        CREATE TABLE IF NOT EXISTS down (\
            column_0 TEXT NOT NULL,\
            column_1 TEXT NOT NULL\
        )\
    ")
    down = [
        ('l', 'f'),
        ('m', 'f'),
        ('g', 'b'),
        ('h', 'c'),
        ('i', 'd'),
        ('p', 'k'),
    ]
    for src, tgt in down:
        cursor.execute("INSERT INTO down VALUES (?, ?)", (src, tgt))
    conn.commit()
    conn.close()


if __name__ == '__main__':
    main()