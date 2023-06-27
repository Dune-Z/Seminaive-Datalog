import matplotlib.pyplot as plt


def main():
    X = [1, 2, 3, 4, 5, 6]
    set_A = [0.695, 1.358, 3.232, 8.873, 24.295, 79.835]
    set_B = [0.109, 0.294, 0.042, 1.207, 2.896, 4.527]
    annotations = [
    "(100, 1000, ~38000)",
    "(200, 1000, ~120000)",
    "(500, 1000, ~250000)",
    "(500, 2000, ~650000)",
    "(500, 5000, ~1000000)",
    "(500, 10000, ~1250000)"
    ]
    plt.figure(figsize=(10, 9))
    plt.title('Benchmark')
    plt.plot(X, set_A, marker='o', label="Amoeba")
    plt.plot(X, set_B, marker='o', label="Crepe")
    plt.xlabel('Index')
    plt.ylabel('Time (second)')
    plt.ylim(0, 85)
    plt.xlim(0, 6.5)
    last_y = 0
    for i in range(len(X)):
        last_y = max(set_A[i], set_B[i], last_y) + 2
        plt.annotate(annotations[i], (X[i]-1, last_y))
    plt.legend()
    plt.savefig('benchmark.png')


if __name__ == '__main__':
    main()
