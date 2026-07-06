# ESQUELETO (Fase 6 Onda 8) — NÃO É INFRA DE PRODUÇÃO.
#
# O Forge é local-first; não há alvo de deploy hospedado hoje. Este arquivo é um
# ponto de partida honesto para quando houver um (ver infra/README.md). Nada aqui
# provisiona recurso real — os providers/recursos ficam comentados até existir um
# alvo, para não virar terraform decorativo.

terraform {
  required_version = ">= 1.5"

  # Definir quando houver um alvo real. Exemplo:
  # required_providers {
  #   aws = {
  #     source  = "hashicorp/aws"
  #     version = "~> 5.0"
  #   }
  # }
}

# Variáveis e recursos de exemplo (comentados — sem alvo real ainda):
#
# variable "region" {
#   description = "Região do alvo de deploy"
#   type        = string
#   default     = "us-east-1"
# }
#
# resource "null_resource" "forge_placeholder" {
#   # Substituir por um recurso real (VM/container/serverless) quando houver alvo.
# }
