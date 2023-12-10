%token a
%%
A -> (): B a {};
B -> (): a | a a {};
