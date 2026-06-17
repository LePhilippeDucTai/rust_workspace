/* M24.3 — PROC NPAR1WAY : Wilcoxon rank-sum (2 groups), Kruskal-Wallis (3).
   Oracles:
   - Wilcoxon height by sex (F first by sas_cmp, n_F=9, n_M=10, n=19; ties at
     62.5 and 66.5 -> midranks): Sum of Scores F=73, Expected=90, z=-1.3892,
     two-sided p~0.1648 (NO continuity correction in v1 -> differs from SAS's
     corrected p; documented). Chi-Square (z^2)=1.930, df1.
   - Kruskal-Wallis on a clean 3-group dataset (ranks 1..9, no ties):
     rank sums 6/15/24, H=7.2, df=2, Pr>Chi-Square~0.0273. */

libname d 'data';

title "Wilcoxon rank-sum: height by sex";
proc npar1way data=d.class wilcoxon;
    class sex;
    var height;
run;

data kw;
    input grp x;
    datalines;
1 1
1 2
1 3
2 4
2 5
2 6
3 7
3 8
3 9
;
run;

title "Kruskal-Wallis: 3 groups";
proc npar1way data=kw;
    class grp;
    var x;
run;
title;
