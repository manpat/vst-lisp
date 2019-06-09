(gain 0.3)

(let env (env-adsr 0.05 0.1 0.8 0.5 (key-vel)))
(let osc (sin (key-freq)))
(output (* env osc))
