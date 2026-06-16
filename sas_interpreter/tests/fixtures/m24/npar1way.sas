/* M24.3 — PROC NPAR1WAY : Wilcoxon (2 groupes) + Kruskal-Wallis (3 groupes).
   Valeurs vérifiées à la main / contre R (voir PROGRESS.md M24.3). */

libname d 'data';

/* Wilcoxon rang-sum : taille selon le sexe (k=2). SAS imprime aussi
   Kruskal-Wallis pour k=2. F=9 filles, M=10 garçons (N=19). */
title "Wilcoxon Rank-Sum: Height by Sex";
proc npar1way data=d.class wilcoxon;
    class sex;
    var height;
run;

/* Kruskal-Wallis 3 groupes en séparation parfaite :
   A{10,12,15} rangs 1-3, B{20,22,25} rangs 4-6, C{28,30,35} rangs 7-9.
   R = 6/15/24, H = 12/(9*10)*(36/3+225/3+576/3) - 30 = 7.2, df=2, p=0.0273. */
data scores;
    input grp $ score;
    datalines;
A 10
A 12
A 15
B 20
B 22
B 25
C 28
C 30
C 35
;
run;

title "Kruskal-Wallis: 3 groups (perfect separation)";
proc npar1way data=scores wilcoxon;
    class grp;
    var score;
run;
title;
