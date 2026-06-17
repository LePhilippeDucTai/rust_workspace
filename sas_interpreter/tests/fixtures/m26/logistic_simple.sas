/* PROC LOGISTIC - binary logistic regression */

data logistic_data;
  input x event @@;
  datalines;
  1 0
  2 0
  2 1
  3 1
  4 1
  4 1
  5 1
  ;
run;

proc logistic data=logistic_data;
  model event = x;
  oddsratio x;
  title "Logistic Regression";
run;
