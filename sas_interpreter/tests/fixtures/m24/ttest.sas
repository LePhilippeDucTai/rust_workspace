/* M24.2 — PROC TTEST : un échantillon, deux échantillons (CLASS), apparié.
   Données : clone de sashelp.class (d.class) + un petit jeu apparié inline.
   Valeurs vérifiées à la main / contre SAS (voir PROGRESS.md M24.2). */

libname d 'data';

/* 1-échantillon : la taille moyenne vaut-elle 60 ?
   sashelp.class height : N=19, mean=62.337, std=5.127, stderr=1.176.
   t = (62.337 - 60) / 1.176 = 1.987, df=18. */
title "One-Sample t-test: Height vs H0=60";
proc ttest data=d.class h0=60;
    var height;
run;

/* 2-échantillons : taille selon le sexe (pooled + Satterthwaite + F plié). */
title "Two-Sample t-test: Height by Sex";
proc ttest data=d.class;
    class sex;
    var height;
run;

/* Apparié : poids avant/après (5 sujets). Différences = -5,-7,-2,+2,-8.
   moyenne diff = -4, test t 1-échantillon sur les différences. */
data weights;
    input before after;
    datalines;
120 115
135 128
140 138
118 120
150 142
;
run;

title "Paired t-test: before vs after weight";
proc ttest data=weights;
    paired before*after;
run;
title;
