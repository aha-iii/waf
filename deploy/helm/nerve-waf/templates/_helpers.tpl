{{- define "nerve-waf.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "nerve-waf.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{- define "nerve-waf.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "nerve-waf.labels" -}}
helm.sh/chart: {{ include "nerve-waf.chart" . }}
{{ include "nerve-waf.selectorLabels" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "nerve-waf.selectorLabels" -}}
app.kubernetes.io/name: {{ include "nerve-waf.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "nerve-waf.controller.fullname" -}}
{{- printf "%s-controller" (include "nerve-waf.fullname" .) | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "nerve-waf.agent.fullname" -}}
{{- printf "%s-agent" (include "nerve-waf.fullname" .) | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "nerve-waf.controller.url" -}}
http://{{ include "nerve-waf.controller.fullname" . }}:{{ .Values.controller.service.port }}
{{- end }}

{{- define "nerve-waf.image" -}}
{{- $reg := .Values.global.imageRegistry | default "" -}}
{{- $repo := .Values.image.repository -}}
{{- if $reg -}}
{{- printf "%s/%s" $reg $repo | replace "//" "/" -}}
{{- else -}}
{{- $repo -}}
{{- end }}
{{- end }}
