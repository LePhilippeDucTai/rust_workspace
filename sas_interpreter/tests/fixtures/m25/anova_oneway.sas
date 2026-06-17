/* PROC ANOVA - one-way analysis of variance */

data anova_data;
  input group $ y @@;
  datalines;
  A 10.2
  A 11.1
  A 10.8
  B 15.3
  B 14.9
  B 15.5
  C 12.0
  C 12.5
  C 11.8
  ;
run;

proc anova data=anova_data;
  class group;
  model y = group;
  means group;
  title "One-Way ANOVA";
run;
