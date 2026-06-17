/* PROC GLM - two-way ANOVA with LSMEANS */

data glm_data;
  input factor1 $ factor2 $ response @@;
  datalines;
  A X 100.2
  A X 101.5
  A Y 95.3
  A Y 96.1
  B X 110.1
  B X 111.2
  B Y 85.4
  B Y 84.9
  ;
run;

proc glm data=glm_data;
  class factor1 factor2;
  model response = factor1 factor2 factor1*factor2;
  lsmeans factor1*factor2;
  title "Two-Way ANOVA with GLM";
run;
