#include <stdio.h>
#include <stdlib.h>

struct Tree {
  struct Tree *left;
  struct Tree *right;
};

struct Tree *make_tree(int depth) {
  struct Tree *node = malloc(sizeof(struct Tree));
  if (depth == 0) {
    node->left = NULL;
    node->right = NULL;
    return node;
  }
  node->left = make_tree(depth - 1);
  node->right = make_tree(depth - 1);
  return node;
}

void free_tree(struct Tree *node) {
  if (node == NULL) {
    return;
  }
  free_tree(node->left);
  free_tree(node->right);
  free(node);
}

int check_tree(struct Tree *node) {
  if (node == NULL) {
    return 0;
  }
  return 1 + check_tree(node->left) + check_tree(node->right);
}

unsigned int_pow(unsigned base, unsigned exp) {
  if (exp == 0) {
    return 1;
  }
  return base * int_pow(base, exp - 1);
}

int main(int argc, char *argv[]) {
  int min_depth = 4;
  int arg = atoi(argv[1]);
  int max_depth;
  if (min_depth + 2 >= arg) {
    max_depth = min_depth + 2;
  } else {
    max_depth = arg;
  }
  int stretch_depth = max_depth + 1;

  struct Tree *stretch_tree = make_tree(stretch_depth);
  printf("stretch tree of depth %d\t check:%d\n", stretch_depth,
         check_tree(stretch_tree));
  free_tree(stretch_tree);

  struct Tree *long_lived_tree = make_tree(max_depth);

  int iterations = int_pow(2, max_depth);

  for (int depth = min_depth; depth < stretch_depth; depth += 2) {
    int check = 0;
    for (int i = 1; i < iterations + 1; i++) {
      struct Tree *tree = make_tree(depth);
      check += check_tree(tree);
      free_tree(tree);
    }

    printf("%d\t trees of depth %d\t check:%d\n", iterations, depth, check);
    iterations /= 4;
  }

  printf("long lived tree of depth%d\t check:%d\n", max_depth,
         check_tree(long_lived_tree));
  free_tree(long_lived_tree);
  return 0;
}